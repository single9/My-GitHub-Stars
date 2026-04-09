use std::path::PathBuf;

use crate::storage::{CategoryRow, Database, RepoRow};

/// Open a fresh DB connection inside `spawn_blocking` (Connection is !Send).
/// All helpers follow this pattern: take PathBuf, return data.

pub async fn load_stats(db_path: PathBuf) -> (i64, i64) {
    tokio::task::spawn_blocking(move || -> Option<(i64, i64)> {
        let db = Database::open(&db_path).ok()?;
        let repos = db.count_repos().ok()?;
        let cats = db.count_categories().ok()?;
        Some((repos, cats))
    })
    .await
    .unwrap_or(None)
    .unwrap_or((0, 0))
}

pub async fn load_categories(db_path: PathBuf) -> Vec<CategoryRow> {
    tokio::task::spawn_blocking(move || -> Option<Vec<CategoryRow>> {
        let db = Database::open(&db_path).ok()?;
        db.get_categories().ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or_default()
}

pub async fn load_repos_for_category(db_path: PathBuf, category_id: i64) -> Vec<RepoRow> {
    tokio::task::spawn_blocking(move || -> Option<Vec<RepoRow>> {
        let db = Database::open(&db_path).ok()?;
        db.get_repos_by_category(category_id).ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or_default()
}

pub async fn search_repos(db_path: PathBuf, query: String) -> Vec<RepoRow> {
    tokio::task::spawn_blocking(move || -> Option<Vec<RepoRow>> {
        let db = Database::open(&db_path).ok()?;
        db.search_repos(&query).ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or_default()
}

pub async fn get_all_repos(db_path: PathBuf) -> Vec<RepoRow> {
    tokio::task::spawn_blocking(move || -> Option<Vec<RepoRow>> {
        let db = Database::open(&db_path).ok()?;
        db.get_all_repos().ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or_default()
}

pub async fn get_repos_by_full_names(db_path: PathBuf, names: Vec<String>) -> Vec<RepoRow> {
    tokio::task::spawn_blocking(move || -> Option<Vec<RepoRow>> {
        let db = Database::open(&db_path).ok()?;
        db.get_repos_by_full_names(&names).ok()
    })
    .await
    .unwrap_or(None)
    .unwrap_or_default()
}

/// Store all fetched repos and classify them.  Returns (total_repos, total_cats).
pub async fn store_repos(
    db_path: PathBuf,
    repos: Vec<crate::api::StarredRepo>,
) -> Result<(i64, i64), String> {
    tokio::task::spawn_blocking(move || -> Result<(i64, i64), String> {
        let db = Database::open(&db_path).map_err(|e| e.to_string())?;
        for repo in &repos {
            let row_id = db.upsert_repo(repo).map_err(|e| e.to_string())?;
            crate::classifier::Classifier::classify_and_store(&db, &[(row_id, repo)])
                .map_err(|e| e.to_string())?;
        }
        let total_repos = db.count_repos().unwrap_or(0);
        let total_cats = db.count_categories().unwrap_or(0);
        Ok((total_repos, total_cats))
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()))
}
