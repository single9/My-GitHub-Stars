use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::storage::RepoRow;

pub const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";
pub const DEFAULT_MODEL: &str = "gpt-4o-mini";
pub const COPILOT_BASE_URL: &str = "https://api.githubcopilot.com";
pub const COPILOT_DEFAULT_MODEL: &str = "gpt-4o-mini";

/// How the client authenticates with the LLM backend.
enum AiAuth {
    /// A plain API key sent as `Authorization: Bearer <key>`.
    ApiKey(String),
    /// A GitHub OAuth token that is exchanged for a short-lived Copilot token
    /// before every request.
    GitHubCopilot(String),
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

#[derive(Deserialize)]
struct CopilotTokenResponse {
    token: String,
}

// ── Implementation ────────────────────────────────────────────────────────────

impl AiClient {
    /// Build a client that authenticates with a plain OpenAI-compatible API key.
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

    /// Build a client that uses a GitHub token to call the GitHub Copilot API.
    /// The GitHub token is exchanged for a short-lived Copilot token on each request.
    pub fn new_copilot(github_token: &str, model: Option<&str>) -> Self {
        Self {
            auth: AiAuth::GitHubCopilot(github_token.to_string()),
            base_url: COPILOT_BASE_URL.to_string(),
            model: model.unwrap_or(COPILOT_DEFAULT_MODEL).to_string(),
            http: Client::new(),
        }
    }

    /// Resolve the bearer token to use for the chat completions request.
    /// For Copilot mode this exchanges the GitHub token for a short-lived token.
    async fn resolve_bearer_token(&self) -> Result<String> {
        match &self.auth {
            AiAuth::ApiKey(key) => Ok(key.clone()),
            AiAuth::GitHubCopilot(github_token) => {
                let resp = self
                    .http
                    .get("https://api.github.com/copilot_internal/v2/token")
                    .header("Authorization", format!("token {}", github_token))
                    .header("User-Agent", "github-stars-pocket")
                    .send()
                    .await?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    let text = resp.text().await.unwrap_or_default();
                    bail!(
                        "Failed to get Copilot token ({}): {}. \
                         Make sure your GitHub account has an active Copilot subscription.",
                        status,
                        text
                    );
                }

                let token_resp: CopilotTokenResponse = resp.json().await?;
                Ok(token_resp.token)
            }
        }
    }

    /// Send a natural-language query along with the full repo list to the LLM.
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

        let bearer = self.resolve_bearer_token().await?;

        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&bearer)
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

        // Strip potential markdown code fences the model might add
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
