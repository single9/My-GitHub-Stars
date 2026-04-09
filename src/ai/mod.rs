use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::storage::RepoRow;

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
/// GitHub Models API — OpenAI-compatible, authenticated with a GitHub PAT.
/// Copilot subscribers get higher rate limits; also available on the free tier.
pub const COPILOT_BASE_URL: &str = "https://models.inference.ai.azure.com";
pub const COPILOT_DEFAULT_MODEL: &str = "gpt-4o-mini";

enum AiAuth {
    ApiKey(String),
    /// GitHub PAT used directly as a bearer token (no exchange needed).
    GitHubPat(String),
}

pub struct AiClient {
    auth: AiAuth,
    base_url: String,
    model: String,
    http: Client,
}

// ── Wire types ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Message,
}

// ── Implementation ────────────────────────────────────────────────────────────

impl AiClient {
    /// OpenAI-compatible client authenticated with a plain API key.
    pub fn new(api_key: &str, base_url: Option<&str>, model: Option<&str>) -> Self {
        Self {
            auth: AiAuth::ApiKey(api_key.to_string()),
            base_url: base_url
                .unwrap_or(DEFAULT_BASE_URL)
                .trim_end_matches('/')
                .to_string(),
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            http: Client::new(),
        }
    }

    /// GitHub Models client — uses a GitHub PAT directly as the bearer token.
    /// Endpoint: https://models.inference.ai.azure.com (OpenAI-compatible).
    pub fn new_copilot(github_pat: &str, model: Option<&str>) -> Self {
        Self {
            auth: AiAuth::GitHubPat(github_pat.to_string()),
            base_url: COPILOT_BASE_URL.to_string(),
            model: model.unwrap_or(COPILOT_DEFAULT_MODEL).to_string(),
            http: Client::new(),
        }
    }

    fn bearer_token(&self) -> &str {
        match &self.auth {
            AiAuth::ApiKey(k) => k,
            AiAuth::GitHubPat(t) => t,
        }
    }

    /// Send a natural-language query with the full repo list to the LLM.
    /// Returns a list of `full_name` strings for the most relevant repos.
    pub async fn search(&self, query: &str, repos: &[RepoRow]) -> Result<Vec<String>> {
        let repo_list: String = repos
            .iter()
            .map(|r| {
                let desc: String = r
                    .description
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(120)
                    .collect();
                let lang = r.language.as_deref().unwrap_or("");
                let topics = r.topics();
                let topics_str = if topics.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", topics.join(", "))
                };
                format!("- {}: {} ({}{})\n", r.full_name, desc, lang, topics_str)
            })
            .collect();

        let user_content = format!(
            "Search query: \"{query}\"\n\nRepositories:\n{repo_list}\n\
             Return a JSON array of full_names (e.g. [\"owner/repo\"]) for the \
             repositories most relevant to the query, ordered by relevance. \
             Return only the JSON array, no explanation."
        );

        let req = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a GitHub repository search assistant. \
                              Given a list of repositories and a natural-language \
                              search query, identify the most relevant repositories. \
                              Respond with a JSON array of repository full_names only."
                        .to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_content,
                },
            ],
            temperature: 0.2,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(self.bearer_token())
            .header("User-Agent", "github-stars-pocket")
            .json(&req)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            bail!("AI API error {}: {}", status, text);
        }

        let chat_resp: ChatResponse = resp.json().await?;
        let content = chat_resp
            .choices
            .first()
            .map(|c| c.message.content.as_str())
            .unwrap_or("[]");

        let json_str = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        let names: Vec<String> = serde_json::from_str(json_str).unwrap_or_default();
        Ok(names)
    }
}
