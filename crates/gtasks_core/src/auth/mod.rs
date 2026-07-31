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

pub async fn authenticate() -> crate::Result<TokenResponse> {
    let (client_id, client_secret) = resolve_client_credentials()?;

    // Start a simple HTTP server to listen for the OAuth callback
    let listener = match TcpListener::bind("127.0.0.1:8080").await {
        Ok(l) => l,
        Err(_) => TcpListener::bind("127.0.0.1:0").await?,
    };
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{}/callback", port);
    let scope = "https://www.googleapis.com/auth/tasks";

    // Generate PKCE code_verifier and code_challenge
    let pkce = generate_pkce();

    // Construct the authorization URL with PKCE parameters
    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256&state={}",
        client_id, redirect_uri, scope, pkce.code_challenge, pkce.state
    );

    tracing::info!("Please open the following URL in your browser to authenticate:");
    open::that(&auth_url)?;
    tracing::info!("Listening for OAuth callback on {}...", redirect_uri);

    let (mut socket, _) = listener.accept().await?;
    let mut buffer = [0; 2048];
    let _ = socket.read(&mut buffer).await?;
    let request_str = String::from_utf8_lossy(&buffer);

    // Extract the authorization code and state from the request
    let (code, returned_state) = extract_code_and_state_from_request(&request_str)
        .ok_or("Failed to extract authorization code from browser callback")?;

    if returned_state != pkce.state {
        return Err("OAuth State mismatch".into());
    }

    let http_response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Authentication successful! You can close this window.</h1></body></html>";
    socket.write_all(http_response.as_bytes()).await?;

    // Exchange the authorization code for an access token using PKCE verifier
    tracing::info!("Exchanging authorization code for access token...");
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
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
