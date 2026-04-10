use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use std::path::Path;

use crate::api::StarredRepo;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open SQLite database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .context("Failed to set PRAGMA")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn
            .execute_batch(
                "
            CREATE TABLE IF NOT EXISTS repos (
                id          INTEGER PRIMARY KEY,
                github_id   INTEGER UNIQUE NOT NULL,
                name        TEXT NOT NULL,
                full_name   TEXT NOT NULL,
                owner       TEXT NOT NULL,
                description TEXT,
                language    TEXT,
                url         TEXT NOT NULL,
                stars_count INTEGER DEFAULT 0,
                topics      TEXT NOT NULL DEFAULT '[]',
                starred_at  TEXT NOT NULL,
                updated_at  TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS categories (
                id            INTEGER PRIMARY KEY,
                name          TEXT UNIQUE NOT NULL,
                category_type TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS repo_categories (
                repo_id     INTEGER NOT NULL REFERENCES repos(id) ON DELETE CASCADE,
                category_id INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
                PRIMARY KEY (repo_id, category_id)
            );

            CREATE INDEX IF NOT EXISTS idx_repos_language ON repos(language);
            CREATE INDEX IF NOT EXISTS idx_repos_starred_at ON repos(starred_at);
            ",
            )
            .context("Failed to run database migration")
    }

    pub fn upsert_repo(&self, repo: &StarredRepo) -> Result<i64> {
        let topics = serde_json::to_string(&repo.topics)?;
        self.conn
            .execute(
                "INSERT INTO repos
                    (github_id, name, full_name, owner, description, language, url,
                     stars_count, topics, starred_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
                 ON CONFLICT(github_id) DO UPDATE SET
                    name=excluded.name, full_name=excluded.full_name,
                    description=excluded.description, language=excluded.language,
                    stars_count=excluded.stars_count, topics=excluded.topics,
                    updated_at=excluded.updated_at",
                params![
                    repo.github_id,
                    repo.name,
                    repo.full_name,
                    repo.owner,
                    repo.description,
                    repo.language,
                    repo.html_url,
                    repo.stargazers_count,
                    topics,
                    repo.starred_at.to_rfc3339(),
                    repo.updated_at.to_rfc3339(),
                ],
            )
            .context("Failed to upsert repo")?;

        let row_id: i64 = self.conn.query_row(
            "SELECT id FROM repos WHERE github_id = ?1",
            params![repo.github_id],
            |row| row.get(0),
        )?;
        Ok(row_id)
    }

    pub fn upsert_category(&self, name: &str, category_type: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT OR IGNORE INTO categories (name, category_type) VALUES (?1, ?2)",
            params![name, category_type],
        )?;
        let id: i64 = self.conn.query_row(
            "SELECT id FROM categories WHERE name = ?1",
            params![name],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn link_repo_category(&self, repo_id: i64, category_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO repo_categories (repo_id, category_id) VALUES (?1, ?2)",
            params![repo_id, category_id],
        )?;
        Ok(())
    }

    pub fn get_categories(&self) -> Result<Vec<CategoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.category_type, COUNT(rc.repo_id) as count
             FROM categories c
             LEFT JOIN repo_categories rc ON rc.category_id = c.id
             GROUP BY c.id
             ORDER BY count DESC, c.name ASC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(CategoryRow {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    category_type: row.get(2)?,
                    count: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to fetch categories")?;
        Ok(rows)
    }

    pub fn get_repos_by_category(&self, category_id: i64) -> Result<Vec<RepoRow>> {
        self.query_repos(
            "SELECT r.id, r.github_id, r.name, r.full_name, r.owner, r.description,
                    r.language, r.url, r.stars_count, r.topics, r.starred_at
             FROM repos r
             JOIN repo_categories rc ON rc.repo_id = r.id
             WHERE rc.category_id = ?1
             ORDER BY r.stars_count DESC",
            params![category_id],
        )
    }

    pub fn search_repos(&self, query: &str) -> Result<Vec<RepoRow>> {
        let pattern = format!("%{}%", query.to_lowercase());
        self.query_repos(
            "SELECT r.id, r.github_id, r.name, r.full_name, r.owner, r.description,
                    r.language, r.url, r.stars_count, r.topics, r.starred_at
             FROM repos r
             WHERE LOWER(r.name) LIKE ?1
                OR LOWER(r.description) LIKE ?1
                OR LOWER(r.topics) LIKE ?1
                OR LOWER(r.full_name) LIKE ?1
             ORDER BY r.stars_count DESC
             LIMIT 200",
            params![pattern],
        )
    }

    /// Fetch repos whose `full_name` appears in `names`, preserving the order of `names`.
    pub fn get_repos_by_full_names(&self, names: &[String]) -> Result<Vec<RepoRow>> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        // Fetch all matching rows in a single query, then re-order by the provided list.
        let placeholders = names
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT r.id, r.github_id, r.name, r.full_name, r.owner, r.description,
                    r.language, r.url, r.stars_count, r.topics, r.starred_at
             FROM repos r
             WHERE r.full_name IN ({placeholders})"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::ToSql> = names
            .iter()
            .map(|n| n as &dyn rusqlite::ToSql)
            .collect();
        let rows = stmt
            .query_map(params.as_slice(), |row| {
                Ok(RepoRow {
                    id: row.get(0)?,
                    github_id: row.get(1)?,
                    name: row.get(2)?,
                    full_name: row.get(3)?,
                    owner: row.get(4)?,
                    description: row.get(5)?,
                    language: row.get(6)?,
                    url: row.get(7)?,
                    stars_count: row.get(8)?,
                    topics_json: row.get(9)?,
                    starred_at: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to query repos by full_names")?;

        // Re-order to match the AI-ranked order
        let mut ordered: Vec<RepoRow> = Vec::with_capacity(rows.len());
        for name in names {
            if let Some(r) = rows.iter().find(|r| &r.full_name == name) {
                ordered.push(r.clone());
            }
        }
        Ok(ordered)
    }

    pub fn get_all_repos(&self) -> Result<Vec<RepoRow>> {
        self.query_repos(
            "SELECT r.id, r.github_id, r.name, r.full_name, r.owner, r.description,
                    r.language, r.url, r.stars_count, r.topics, r.starred_at
             FROM repos r
             ORDER BY r.starred_at DESC",
            [],
        )
    }

    pub fn count_repos(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM repos", [], |r| r.get(0))?)
    }

    pub fn count_categories(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?)
    }

    fn query_repos(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<RepoRow>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt
            .query_map(params, |row| {
                Ok(RepoRow {
                    id: row.get(0)?,
                    github_id: row.get(1)?,
                    name: row.get(2)?,
                    full_name: row.get(3)?,
                    owner: row.get(4)?,
                    description: row.get(5)?,
                    language: row.get(6)?,
                    url: row.get(7)?,
                    stars_count: row.get(8)?,
                    topics_json: row.get(9)?,
                    starred_at: row.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .context("Failed to query repos")?;
        Ok(rows)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub category_type: String,
    pub count: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepoRow {
    pub id: i64,
    pub github_id: i64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub url: String,
    pub stars_count: i64,
    pub topics_json: String,
    pub starred_at: String,
}

impl RepoRow {
    pub fn topics(&self) -> Vec<String> {
        serde_json::from_str(&self.topics_json).unwrap_or_default()
    }
}
