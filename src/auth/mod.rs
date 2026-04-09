use anyhow::{bail, Context, Result};
use serde::Deserialize;

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const SCOPE: &str = "read:user";

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// GitHub may return an error object instead of a success payload.
#[derive(Debug, Deserialize)]
struct ApiError {
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub struct AuthClient {
    http: reqwest::Client,
}

impl AuthClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    pub async fn request_device_code(&self, client_id: &str) -> Result<DeviceCodeResponse> {
        let response = self
            .http
            .post(DEVICE_CODE_URL)
            .header("Accept", "application/json")
            .form(&[("client_id", client_id), ("scope", SCOPE)])
            .send()
            .await
            .context("Failed to connect to GitHub. Check your internet connection.")?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("Failed to read GitHub response")?;

        if !status.is_success() {
            // Try to surface GitHub's error message
            if let Ok(e) = serde_json::from_str::<ApiError>(&body) {
                if let Some(desc) = e.error_description {
                    bail!("GitHub OAuth error: {}", desc);
                }
                if let Some(err) = e.error {
                    bail!("GitHub OAuth error: {}", err);
                }
            }
            bail!("GitHub returned HTTP {} — check your OAuth App Client ID in config.", status);
        }

        serde_json::from_str::<DeviceCodeResponse>(&body).with_context(|| {
            // Include the raw body so the user can see what GitHub returned
            format!(
                "Failed to parse GitHub device code response.\n\
                 Raw response: {}\n\
                 Hint: Ensure your OAuth App has 'Device Flow' enabled and the Client ID is correct.",
                body
            )
        })
    }

    /// Polls GitHub for token.
    /// Returns Ok(PollResult) or Err on fatal errors.
    pub async fn poll_for_token(&self, client_id: &str, device_code: &str) -> Result<PollResult> {
        let response = self
            .http
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", client_id),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .context("Failed to poll for token")?;

        let body: TokenResponse = response
            .json()
            .await
            .context("Failed to parse token response")?;

        if let Some(token) = body.access_token {
            if !token.is_empty() {
                return Ok(PollResult::Token(token));
            }
        }

        match body.error.as_deref() {
            Some("authorization_pending") => Ok(PollResult::Pending),
            Some("slow_down") => Ok(PollResult::SlowDown),
            Some("expired_token") => bail!("Device code expired. Please restart authentication."),
            Some("access_denied") => bail!("Access denied by user."),
            Some(e) => bail!(
                "OAuth error: {} — {}",
                e,
                body.error_description.unwrap_or_default()
            ),
            None => Ok(PollResult::Pending),
        }
    }
}

pub enum PollResult {
    Pending,
    SlowDown,
    Token(String),
}

