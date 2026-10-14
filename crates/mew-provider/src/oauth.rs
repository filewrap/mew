use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::time::timeout;
use url::Url;

const AUTHORIZATION_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const OAUTH_CLIENT_ID_ENV: &str = "MEW_GEMINI_OAUTH_CLIENT_ID";
const OAUTH_CLIENT_SECRET_ENV: &str = "MEW_GEMINI_OAUTH_CLIENT_SECRET";
const OAUTH_PROJECT_ID_ENV: &str = "MEW_GEMINI_OAUTH_PROJECT_ID";
const TOKEN_FILE: &str = "gemini-oauth.json";
const SCOPES: &str = "https://www.googleapis.com/auth/cloud-platform https://www.googleapis.com/auth/generative-language.retriever";

#[derive(Debug, Clone)]
pub struct GeminiOAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub project_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenStore {
    refresh_token: String,
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    expires_at: u64,
    #[serde(default)]
    token_type: String,
    #[serde(default)]
    scope: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub fn gemini_oauth_credentials() -> Result<GeminiOAuthCredentials> {
    let client_id = std::env::var(OAUTH_CLIENT_ID_ENV)
        .map_err(|_| anyhow!("set {OAUTH_CLIENT_ID_ENV} to your Google Desktop OAuth client ID"))?;
    let client_secret = std::env::var(OAUTH_CLIENT_SECRET_ENV).map_err(|_| {
        anyhow!("set {OAUTH_CLIENT_SECRET_ENV} to your Google Desktop OAuth client secret")
    })?;
    let project_id = std::env::var(OAUTH_PROJECT_ID_ENV)
        .map_err(|_| anyhow!("set {OAUTH_PROJECT_ID_ENV} to your Google Cloud project ID"))?;
    for (name, value) in [
        (OAUTH_CLIENT_ID_ENV, client_id.as_str()),
        (OAUTH_CLIENT_SECRET_ENV, client_secret.as_str()),
        (OAUTH_PROJECT_ID_ENV, project_id.as_str()),
    ] {
        if value.trim().is_empty() {
            bail!("{name} must not be empty");
        }
    }
    Ok(GeminiOAuthCredentials {
        client_id,
        client_secret,
        project_id,
    })
}

pub fn has_gemini_oauth_token() -> bool {
    gemini_oauth_credentials().is_ok()
        && load_tokens()
            .ok()
            .flatten()
            .is_some_and(|tokens| !tokens.refresh_token.is_empty())
}

pub fn gemini_oauth_token_path() -> Result<PathBuf> {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("APPDATA is not set"))?
    } else if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("HOME is not set"))?
            .join(".config")
    };
    Ok(base.join("mew").join(TOKEN_FILE))
}

fn load_tokens() -> Result<Option<TokenStore>> {
    let path = gemini_oauth_token_path()?;
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to read a symlinked Gemini OAuth token file");
    }
    match fs::read(&path) {
        Ok(data) => {
            if data.len() > 64 * 1024 {
                bail!("Gemini OAuth token file is unexpectedly large");
            }
            Ok(Some(
                serde_json::from_slice(&data).context("parse Gemini OAuth token file")?,
            ))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn save_tokens(tokens: &TokenStore) -> Result<()> {
    let path = gemini_oauth_token_path()?;
    let directory = path
        .parent()
        .ok_or_else(|| anyhow!("missing token directory"))?;
    if fs::symlink_metadata(directory).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to write Gemini OAuth tokens through a symlinked config directory");
    }
    fs::create_dir_all(directory)?;
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        bail!("refusing to replace a symlinked Gemini OAuth token file");
    }
    let temp = directory.join(format!(
        ".gemini-oauth-{}-{}.tmp",
        std::process::id(),
        now_seconds()
    ));
    let data = serde_json::to_vec_pretty(tokens)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(&data)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path)?;
    }
    if let Err(error) = fs::rename(&temp, &path) {
        let _ = fs::remove_file(&temp);
        return Err(error)
            .with_context(|| format!("save Gemini OAuth tokens to {}", path.display()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn logout_gemini_oauth() -> Result<bool> {
    let path = gemini_oauth_token_path()?;
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("refusing to remove a symlinked Gemini OAuth token file")
        }
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub async fn gemini_oauth_access_token() -> Result<(String, String)> {
    let credentials = gemini_oauth_credentials()?;
    let mut tokens = load_tokens()?
        .ok_or_else(|| anyhow!("no Gemini OAuth refresh token; run `mew provider login gemini`"))?;
    if tokens.refresh_token.trim().is_empty() {
        bail!("Gemini OAuth refresh token is missing; run `mew provider login gemini`");
    }
    if tokens.expires_at > now_seconds().saturating_add(90) && !tokens.access_token.is_empty() {
        return Ok((tokens.access_token, credentials.project_id));
    }
    let response = reqwest::Client::new()
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("refresh_token", tokens.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .context("refresh Google Gemini OAuth access token")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Google OAuth token refresh failed (HTTP {status}): {}",
            truncate(&body, 500)
        );
    }
    let response: TokenResponse = response
        .json()
        .await
        .context("parse Google OAuth token response")?;
    if let Some(error) = response.error {
        bail!(
            "Google OAuth token refresh failed: {}",
            response.error_description.unwrap_or(error)
        );
    }
    tokens.access_token = response.access_token;
    tokens.expires_at = now_seconds().saturating_add(response.expires_in.unwrap_or(3600));
    tokens.token_type = response.token_type.unwrap_or_else(|| "Bearer".into());
    if let Some(scope) = response.scope {
        tokens.scope = scope;
    }
    if let Some(refresh_token) = response.refresh_token {
        tokens.refresh_token = refresh_token;
    }
    let access_token = tokens.access_token.clone();
    save_tokens(&tokens)?;
    Ok((access_token, credentials.project_id))
}

pub async fn login_gemini_oauth() -> Result<()> {
    let credentials = gemini_oauth_credentials()?;
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .context("start local OAuth callback listener")?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let state = random_string(32);
    let verifier = random_string(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let auth_url = authorization_url(&credentials, &redirect_uri, &state, &challenge)?;
    match open_authorization_url(auth_url.as_str()).await {
        Ok(program) => println!("Opening Google sign-in with the system browser handler ({program})."),
        Err(error) => println!("Could not start a system browser automatically ({error:#}). Open the URL below manually."),
    }
    println!("Google authorization URL (valid for 5 minutes):\n{auth_url}\n");
    let (mut stream, _) = timeout(Duration::from_secs(300), listener.accept())
        .await
        .context("timed out waiting for Google OAuth callback")??;
    let request_target = read_callback_request(&mut stream).await?;
    let callback = Url::parse(&format!("{redirect_uri}{request_target}"))?;
    let params = callback
        .query_pairs()
        .into_owned()
        .collect::<std::collections::HashMap<_, _>>();
    let response_html = if params.contains_key("code") {
        "<h2>Google sign-in received.</h2><p>You can return to Mew.</p>"
    } else {
        "<h2>Google sign-in did not complete.</h2><p>Return to Mew for details.</p>"
    };
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}", response_html.len(), response_html);
    let _ = stream.write_all(response.as_bytes()).await;
    if params.get("state").map(String::as_str) != Some(state.as_str()) {
        bail!("OAuth state validation failed; no token was saved");
    }
    if let Some(error) = params.get("error") {
        bail!("Google authorization failed: {error}");
    }
    let code = params
        .get("code")
        .ok_or_else(|| anyhow!("Google callback did not include an authorization code"))?;
    let response = reqwest::Client::new()
        .post(TOKEN_ENDPOINT)
        .form(&[
            ("client_id", credentials.client_id.as_str()),
            ("client_secret", credentials.client_secret.as_str()),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect_uri.as_str()),
        ])
        .send()
        .await
        .context("exchange Google OAuth authorization code")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!(
            "Google OAuth code exchange failed (HTTP {status}): {}",
            truncate(&body, 500)
        );
    }
    let response: TokenResponse = response
        .json()
        .await
        .context("parse Google OAuth code exchange response")?;
    if let Some(error) = response.error {
        bail!(
            "Google OAuth code exchange failed: {}",
            response.error_description.unwrap_or(error)
        );
    }
    let refresh_token = match response.refresh_token {
        Some(token) if !token.is_empty() => token,
        _ => load_tokens()?.map(|tokens| tokens.refresh_token).filter(|token| !token.is_empty())
            .ok_or_else(|| anyhow!("Google did not return a refresh token; revoke Mew in Google Account permissions and retry login"))?,
    };
    let store = TokenStore {
        refresh_token,
        access_token: response.access_token,
        expires_at: now_seconds().saturating_add(response.expires_in.unwrap_or(3600)),
        token_type: response.token_type.unwrap_or_else(|| "Bearer".into()),
        scope: response.scope.unwrap_or_else(|| SCOPES.to_string()),
    };
    save_tokens(&store)?;
    println!(
        "Gemini OAuth credentials saved privately to {}",
        gemini_oauth_token_path()?.display()
    );
    Ok(())
}

async fn open_authorization_url(url: &str) -> Result<String> {
    let mut last_error = None;
    for (program, arguments) in browser_commands(url) {
        match Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                tokio::spawn(async move {
                    let _ = child.wait().await;
                });
                return Ok(program.to_string());
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error
        .map(anyhow::Error::from)
        .unwrap_or_else(|| anyhow!("no system browser launcher is configured")))
}

fn browser_commands(url: &str) -> Vec<(&'static str, Vec<String>)> {
    #[cfg(target_os = "windows")]
    {
        vec![(
            "rundll32.exe",
            vec!["url.dll,FileProtocolHandler".to_string(), url.to_string()],
        )]
    }
    #[cfg(target_os = "macos")]
    {
        vec![("open", vec![url.to_string()])]
    }
    #[cfg(target_os = "android")]
    {
        vec![
            ("termux-open-url", vec![url.to_string()]),
            ("xdg-open", vec![url.to_string()]),
        ]
    }
    #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
    {
        vec![
            ("xdg-open", vec![url.to_string()]),
            ("gio", vec!["open".to_string(), url.to_string()]),
        ]
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = url;
        Vec::new()
    }
}

async fn read_callback_request(stream: &mut TcpStream) -> Result<String> {
    let mut bytes = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    loop {
        let count = timeout(Duration::from_secs(20), stream.read(&mut chunk))
            .await
            .context("timed out reading OAuth callback")??;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > 8192 {
            bail!("OAuth callback request was too large");
        }
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let text = std::str::from_utf8(&bytes).context("OAuth callback was not valid HTTP text")?;
    let request_line = text
        .lines()
        .next()
        .ok_or_else(|| anyhow!("missing OAuth callback request line"))?;
    let mut parts = request_line.split_whitespace();
    if parts.next() != Some("GET") {
        bail!("unexpected HTTP method on OAuth callback");
    }
    let target = parts
        .next()
        .ok_or_else(|| anyhow!("missing OAuth callback target"))?;
    if !target.starts_with('/') {
        bail!("invalid OAuth callback path");
    }
    Ok(target.to_string())
}

fn authorization_url(
    credentials: &GeminiOAuthCredentials,
    redirect_uri: &str,
    state: &str,
    challenge: &str,
) -> Result<Url> {
    let mut url = Url::parse(AUTHORIZATION_ENDPOINT)?;
    url.query_pairs_mut()
        .append_pair("client_id", &credentials.client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPES)
        .append_pair("state", state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256");
    Ok(url)
}

fn random_string(length: usize) -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn truncate(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_url_uses_google_desktop_oauth_state_and_pkce() {
        let credentials = GeminiOAuthCredentials {
            client_id: "desktop-client.apps.googleusercontent.com".into(),
            client_secret: "test-secret".into(),
            project_id: "test-project".into(),
        };
        let url = authorization_url(
            &credentials,
            "http://127.0.0.1:43210",
            "state-value",
            "challenge-value",
        )
        .unwrap();
        let query = url
            .query_pairs()
            .into_owned()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(url.host_str(), Some("accounts.google.com"));
        assert_eq!(
            query.get("client_id").map(String::as_str),
            Some(credentials.client_id.as_str())
        );
        assert_eq!(
            query.get("redirect_uri").map(String::as_str),
            Some("http://127.0.0.1:43210")
        );
        assert_eq!(query.get("state").map(String::as_str), Some("state-value"));
        assert_eq!(
            query.get("code_challenge").map(String::as_str),
            Some("challenge-value")
        );
        assert_eq!(
            query.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert!(query
            .get("scope")
            .unwrap()
            .contains("generative-language.retriever"));
        assert_eq!(
            query.get("access_type").map(String::as_str),
            Some("offline")
        );
    }

    #[test]
    fn browser_launcher_passes_the_authorization_url_as_an_argument() {
        let url = "https://accounts.google.com/o/oauth2/v2/auth?state=a&client_id=b";
        let commands = browser_commands(url);
        assert!(!commands.is_empty());
        for (_, arguments) in &commands {
            assert_eq!(arguments.last().map(String::as_str), Some(url));
        }
        #[cfg(target_os = "windows")]
        assert_eq!(commands[0].0, "rundll32.exe");
        #[cfg(target_os = "macos")]
        assert_eq!(commands[0].0, "open");
        #[cfg(target_os = "android")]
        assert_eq!(commands[0].0, "termux-open-url");
        #[cfg(all(unix, not(any(target_os = "macos", target_os = "android"))))]
        assert_eq!(commands[0].0, "xdg-open");
    }
}
