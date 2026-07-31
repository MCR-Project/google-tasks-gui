pub mod keyring;

use serde::Deserialize;
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: u64,
    pub refresh_token: Option<String>,
    pub token_type: String,
}

pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_port: Option<u16>,
}

impl OAuthConfig {
    pub fn from_env() -> crate::Result<Self> {
        let (client_id, client_secret) = resolve_client_credentials()?;
        Ok(Self {
            client_id,
            client_secret,
            redirect_port: None,
        })
    }
}

fn resolve_client_credentials() -> crate::Result<(String, String)> {
    let client_id = match option_env!("GOOGLE_CLIENT_ID") {
        Some(val) if !val.is_empty() => val.to_string(),
        _ => env::var("GOOGLE_CLIENT_ID")
            .map_err(|_| crate::GTasksError::Auth("GOOGLE_CLIENT_ID must be set at compile-time or in environment".to_string()))?,
    };
    let client_secret = match option_env!("GOOGLE_CLIENT_SECRET") {
        Some(val) if !val.is_empty() => val.to_string(),
        _ => env::var("GOOGLE_CLIENT_SECRET")
            .map_err(|_| crate::GTasksError::Auth("GOOGLE_CLIENT_SECRET must be set at compile-time or in environment".to_string()))?,
    };
    Ok((client_id, client_secret))
}

// Encode in sha256 URL-safe format without padding
pub fn generate_pkce() -> PkceChallenge {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

    let hash = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let state = URL_SAFE_NO_PAD.encode(bytes);

    PkceChallenge {
        code_verifier,
        code_challenge,
        state,
    }
}

pub async fn authenticate_with_handler<F>(
    config: &OAuthConfig,
    url_handler: F,
) -> crate::Result<TokenResponse>
where
    F: FnOnce(&str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>,
{
    let port = config.redirect_port.unwrap_or(8080);
    let listener = match TcpListener::bind(format!("127.0.0.1:{port}")).await {
        Ok(l) => l,
        Err(_) => TcpListener::bind("127.0.0.1:0").await?,
    };
    let local_port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{local_port}/callback");
    let scope = "https://www.googleapis.com/auth/tasks";

    let pkce = generate_pkce();

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        config.client_id, redirect_uri, scope, pkce.code_challenge, pkce.state
    );

    url_handler(&auth_url).map_err(|e| crate::GTasksError::Auth(e.to_string()))?;
    tracing::info!("Listening for OAuth callback on {}...", redirect_uri);

    let (mut socket, _) = listener.accept().await?;
    let mut buffer = [0; 2048];
    let _ = socket.read(&mut buffer).await?;
    let request_str = String::from_utf8_lossy(&buffer);

    let (code, returned_state) = extract_code_and_state_from_request(&request_str)
        .ok_or_else(|| crate::GTasksError::Auth("Failed to extract authorization code from browser callback".to_string()))?;

    if returned_state != pkce.state {
        return Err(crate::GTasksError::Auth("OAuth State mismatch".to_string()));
    }

    let http_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Authentication successful! You can close this window.</h1></body></html>";
    socket.write_all(http_response.as_bytes()).await?;

    tracing::info!("Exchanging authorization code for access token...");
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("code_verifier", pkce.code_verifier.as_str()),
        ])
        .send()
        .await?
        .json::<TokenResponse>()
        .await?;
    Ok(token_response)
}

pub async fn authenticate() -> crate::Result<TokenResponse> {
    let config = OAuthConfig::from_env()?;
    authenticate_with_handler(&config, |url| {
        tracing::info!("Please open the following URL in your browser to authenticate:");
        let _ = open::that(url);
        Ok(())
    })
    .await
}

fn extract_code_and_state_from_request(request: &str) -> Option<(String, String)> {
    for line in request.lines() {
        if line.starts_with("GET") {
            let code = line
                .split("code=")
                .nth(1)?
                .split('&')
                .next()?
                .split(' ')
                .next()?
                .to_string();
            let state = line
                .split("state=")
                .nth(1)?
                .split('&')
                .next()?
                .split(' ')
                .next()?
                .to_string();
            return Some((code, state));
        }
    }
    None
}

pub async fn refresh_access_token(
    refresh_token: &str,
) -> crate::Result<TokenResponse> {
    let (client_id, client_secret) = resolve_client_credentials()?;

    let client = reqwest::Client::new();
    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token.to_string()),
            ("grant_type", "refresh_token".to_string()),
        ])
        .send()
        .await?
        .json::<TokenResponse>()
        .await?;
    Ok(token_response)
}
