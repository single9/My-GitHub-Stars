use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::watch;

const API_BASE: &str = "https://api.github.com";
const PER_PAGE: u8 = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarredRepo {
    pub github_id: i64,
    pub name: String,
    pub full_name: String,
    pub owner: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub html_url: String,
    pub stargazers_count: i64,
    pub topics: Vec<String>,
    pub starred_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ApiStarredItem {
    starred_at: DateTime<Utc>,
    repo: ApiRepo,
}

#[derive(Debug, Deserialize)]
struct ApiRepo {
    id: i64,
    name: String,
    full_name: String,
    owner: ApiOwner,
    description: Option<String>,
    language: Option<String>,
    html_url: String,
    stargazers_count: i64,
    topics: Option<Vec<String>>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct ApiOwner {
    login: String,
}

pub struct ApiClient {
    http: reqwest::Client,
    token: String,
}

impl ApiClient {
    pub fn new(token: impl Into<String>) -> Self {
        let token = token.into();
        let http = reqwest::Client::builder()
            .user_agent("my-gh-stars/0.1.0")
            .default_headers({
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert(
                    "Authorization",
                    format!("Bearer {}", token).parse().unwrap(),
                );
                headers.insert(
                    "Accept",
                    "application/vnd.github.star+json".parse().unwrap(),
                );
                headers
            })
            .build()
            .expect("Failed to build HTTP client");

        Self { http, token: token.into() }
    }

    pub async fn fetch_all_starred(
        &self,
        progress_tx: Option<watch::Sender<usize>>,
    ) -> Result<Vec<StarredRepo>> {
        let mut all = Vec::new();
        let mut page = 1u32;

        loop {
            let url = format!(
                "{}/user/starred?per_page={}&page={}",
                API_BASE, PER_PAGE, page
            );

            let response = self
                .http
                .get(&url)
                .send()
                .await
                .with_context(|| format!("Failed to fetch starred page {}", page))?;

            if response.status() == 401 {
                anyhow::bail!("GitHub token is invalid or expired. Please re-authenticate.");
            }

            let has_next = response
                .headers()
                .get("Link")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.contains("rel=\"next\""))
                .unwrap_or(false);

            let items: Vec<ApiStarredItem> = response
                .json()
                .await
                .with_context(|| format!("Failed to parse starred page {}", page))?;

            if items.is_empty() {
                break;
            }

            for item in items {
                all.push(StarredRepo {
                    github_id: item.repo.id,
                    name: item.repo.name,
                    full_name: item.repo.full_name,
                    owner: item.repo.owner.login,
                    description: item.repo.description,
                    language: item.repo.language,
                    html_url: item.repo.html_url,
                    stargazers_count: item.repo.stargazers_count,
                    topics: item.repo.topics.unwrap_or_default(),
                    starred_at: item.starred_at,
                    updated_at: item.repo.updated_at,
                });
            }

            // Send non-blocking progress update (watch overwrites, never blocks)
            if let Some(tx) = &progress_tx {
                let _ = tx.send(all.len());
            }

            if !has_next {
                break;
            }
            page += 1;
        }

        Ok(all)
    }

    pub async fn get_current_user(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct User {
            login: String,
        }

        let user: User = self
            .http
            .get(format!("{}/user", API_BASE))
            .send()
            .await
            .context("Failed to get current user")?
            .json()
            .await
            .context("Failed to parse user response")?;

        Ok(user.login)
    }
}
