use anyhow::Result;

use crate::api::StarredRepo;
use crate::storage::Database;

pub struct Classifier;

impl Classifier {
    /// Categorizes a batch of repos and persists the categories + links into the DB.
    pub fn classify_and_store(db: &Database, repos: &[(i64, &StarredRepo)]) -> Result<()> {
        for (row_id, repo) in repos {
            // Categorize by primary language
            if let Some(lang) = &repo.language {
                if !lang.is_empty() {
                    let cat_id = db.upsert_category(lang, "language")?;
                    db.link_repo_category(*row_id, cat_id)?;
                }
            }

            // Categorize by each GitHub topic
            for topic in &repo.topics {
                if !topic.is_empty() {
                    let cat_id = db.upsert_category(topic, "topic")?;
                    db.link_repo_category(*row_id, cat_id)?;
                }
            }

            // Fallback: repos with no language and no topics go to "Uncategorized"
            if repo.language.is_none() && repo.topics.is_empty() {
                let cat_id = db.upsert_category("Uncategorized", "other")?;
                db.link_repo_category(*row_id, cat_id)?;
            }
        }
        Ok(())
    }
}
