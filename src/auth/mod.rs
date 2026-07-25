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
}

// Encode in base 64 URL-safe format without padding
pub fn generate_pkce() -> PkceChallenge {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(bytes);

    let hash = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    PkceChallenge {
        code_verifier,
        code_challenge,
    }
}

pub async fn authenticate() -> Result<TokenResponse, Box<dyn std::error::Error>> {
    let client_id = env::var("GOOGLE_CLIENT_ID")
        .expect("GOOGLE_CLIENT_ID must be set in .env file");
    let client_secret = env::var("GOOGLE_CLIENT_SECRET")
        .expect("GOOGLE_CLIENT_SECRET must be set in .env file");

    let redirect_uri = "http://localhost:8080/callback";
    let scope = "https://www.googleapis.com/auth/tasks";

    // Generate PKCE code_verifier and code_challenge
    let pkce = generate_pkce();

    // Construct the authorization URL with PKCE parameters
    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={}&code_challenge_method=S256",
        client_id, redirect_uri, scope, pkce.code_challenge
    );

    println!("Please open the following URL in your browser to authenticate:");
    open::that(&auth_url)?;
    
    // Start a simple HTTP server to listen for the OAuth callback
    let listener = TcpListener::bind("localhost:8080").await?;
    println!("Listening for OAuth callback on http://localhost:8080/callback...");

    let (mut socket, _) = listener.accept().await?;
    let mut buffer = [0; 2048];
    socket.read(&mut buffer).await?;
    let request_str = String::from_utf8_lossy(&buffer);

    // Extract the authorization code from the request
    let code = extract_code_from_request(&request_str)
            .ok_or("Failed to extract authorization code from browser callback")?;
    
    let http_repsonde = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n<html><body><h1>Authentication successful! You can close this window.</h1></body></html>";
    socket.write_all(http_repsonde.as_bytes()).await?;

    // Exchange the authorization code for an access token using PKCE verifier
    println!("Exchanging authorization code for access token...");
    let client = reqwest::Client::new();
    let token_response = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_uri.to_string()),
            ("grant_type", "authorization_code".to_string()),
            ("code", code),
            ("code_verifier", pkce.code_verifier),
        ])
        .send()
        .await?
        .json::<TokenResponse>()
        .await?;
    Ok(token_response)
}

fn extract_code_from_request(request: &str) -> Option<String> {
    for line in request.lines() {
        if line.starts_with("GET") {
            if let Some(code_param) = line.split("code=").nth(1) {
                let code = code_param.split('&').next()?.split(' ').next()?;
                return Some(code.to_string());
            }
        }
    }
    None
}

pub async fn refresh_access_token(refresh_token: &str) -> Result<TokenResponse, Box<dyn std::error::Error>> {
    let client_id = env::var("GOOGLE_CLIENT_ID")
        .expect("GOOGLE_CLIENT_ID must be set in .env file");
    let client_secret = env::var("GOOGLE_CLIENT_SECRET")
        .expect("GOOGLE_CLIENT_SECRET must be set in .env file");

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