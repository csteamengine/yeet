use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

const GITHUB_CLIENT_ID: &str = "Iv23liBECPgV5fEeu4Rf";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
    pub avatar_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollResult {
    pub status: String,
    pub token: Option<String>,
    pub message: Option<String>,
}

pub struct GitHubAuthState {
    client: reqwest::Client,
    polling: Mutex<bool>,
    token_path: PathBuf,
}

impl GitHubAuthState {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self {
            client: reqwest::Client::new(),
            polling: Mutex::new(false),
            token_path: app_data_dir.join(".github_token"),
        }
    }

    fn store_token(&self, token: &str) -> Result<(), String> {
        std::fs::write(&self.token_path, token).map_err(|e| format!("failed to store token: {}", e))
    }

    fn retrieve_token(&self) -> Result<Option<String>, String> {
        if self.token_path.exists() {
            let token = std::fs::read_to_string(&self.token_path)
                .map_err(|e| format!("failed to read token: {}", e))?;
            if token.trim().is_empty() {
                return Ok(None);
            }
            Ok(Some(token.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    fn delete_token(&self) -> Result<(), String> {
        if self.token_path.exists() {
            std::fs::remove_file(&self.token_path)
                .map_err(|e| format!("failed to delete token: {}", e))?;
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn github_start_device_flow(
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<DeviceFlowResponse, String> {
    let resp = auth
        .client
        .post("https://github.com/login/device/code")
        .header("Accept", "application/json")
        .form(&[("client_id", GITHUB_CLIENT_ID), ("scope", "repo")])
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub returned {}: {}", 400, text));
    }

    let flow: DeviceFlowResponse = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse response: {}", e))?;

    *auth.polling.lock().unwrap() = true;
    Ok(flow)
}

#[tauri::command]
pub async fn github_poll_token(
    auth: tauri::State<'_, GitHubAuthState>,
    device_code: String,
) -> Result<PollResult, String> {
    if !*auth.polling.lock().unwrap() {
        return Ok(PollResult { status: "error".into(), token: None, message: Some("not polling".into()) });
    }

    let resp = auth
        .client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("device_code", device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse response: {}", e))?;

    log::info!("[github_auth] poll response: {}", body);

    if let Some(token) = body.get("access_token").and_then(|v| v.as_str()) {
        *auth.polling.lock().unwrap() = false;
        auth.store_token(token)?;
        log::info!("[github_auth] token stored successfully");
        return Ok(PollResult { status: "success".into(), token: Some(token.to_string()), message: None });
    }

    let error = body
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match error {
        "authorization_pending" | "slow_down" => {
            Ok(PollResult { status: "pending".into(), token: None, message: None })
        }
        "expired_token" => {
            *auth.polling.lock().unwrap() = false;
            Ok(PollResult { status: "expired".into(), token: None, message: None })
        }
        _ => {
            *auth.polling.lock().unwrap() = false;
            Ok(PollResult { status: "error".into(), token: None, message: Some(error.to_string()) })
        }
    }
}

#[tauri::command]
pub async fn github_open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("failed to open URL: {}", e))
}

#[tauri::command]
pub async fn github_cancel_polling(
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<(), String> {
    *auth.polling.lock().unwrap() = false;
    Ok(())
}

#[tauri::command]
pub async fn github_get_token(
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<Option<String>, String> {
    auth.retrieve_token()
}

#[tauri::command]
pub async fn github_get_user(
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<Option<GitHubUser>, String> {
    let token = match auth.retrieve_token()? {
        Some(t) => t,
        None => {
            log::info!("[github_auth] no token found");
            return Ok(None);
        }
    };

    log::info!("[github_auth] fetching user with token (len={})", token.len());

    let resp = auth
        .client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "Yeet")
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    let status = resp.status();
    log::info!("[github_auth] /user response status: {}", status);

    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        log::warn!("[github_auth] /user error body: {}", body);
        if status.as_u16() == 401 {
            auth.delete_token()?;
            return Ok(None);
        }
        return Err(format!("GitHub API error: {}", status));
    }

    let user: GitHubUser = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse user: {}", e))?;

    log::info!("[github_auth] authenticated as {}", user.login);
    Ok(Some(user))
}

#[tauri::command]
pub async fn github_logout(
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<(), String> {
    auth.delete_token()
}

// ---- Auto-update ----

const GITHUB_REPO: &str = "csteamengine/yeet";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateInfo {
    pub available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub release_notes: Option<String>,
}

/// Check GitHub releases API for a newer version.
#[tauri::command]
pub async fn check_for_updates(
    app: tauri::AppHandle,
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<UpdateInfo, String> {
    let current_version = app.config().version.clone().unwrap_or_else(|| "0.0.0".into());

    let mut req = auth
        .client
        .get(format!(
            "https://api.github.com/repos/{}/releases/latest",
            GITHUB_REPO
        ))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "Yeet");

    if let Ok(Some(token)) = auth.retrieve_token() {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("request failed: {}", e))?;

    if resp.status().as_u16() == 404 {
        return Ok(UpdateInfo {
            available: false,
            current_version,
            latest_version: None,
            release_notes: None,
        });
    }

    if !resp.status().is_success() {
        return Err(format!("GitHub API error: {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let tag = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0");
    let latest = tag.trim_start_matches('v');
    let notes = body
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let available = version_gt(latest, &current_version);
    log::info!(
        "[update] current={} latest={} available={}",
        current_version,
        latest,
        available
    );

    Ok(UpdateInfo {
        available,
        current_version,
        latest_version: Some(latest.to_string()),
        release_notes: notes,
    })
}

/// Download, verify, and install the update in-place, then restart.
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    auth: tauri::State<'_, GitHubAuthState>,
) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;

    let mut builder = app.updater_builder();
    if let Ok(Some(token)) = auth.retrieve_token() {
        builder = builder
            .header("Authorization", format!("Bearer {}", token))
            .map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;

    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update available".to_string())?;

    log::info!("[update] downloading v{}", update.version);
    update
        .download_and_install(
            |downloaded, total| {
                if let Some(t) = total {
                    log::info!("[update] progress: {}/{}", downloaded, t);
                }
            },
            || {
                log::info!("[update] download complete, installing…");
            },
        )
        .await
        .map_err(|e| e.to_string())?;

    log::info!("[update] installed, restarting");
    app.restart();
}

fn version_gt(a: &str, b: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let va = parse(a);
    let vb = parse(b);
    for i in 0..va.len().max(vb.len()) {
        let pa = va.get(i).copied().unwrap_or(0);
        let pb = vb.get(i).copied().unwrap_or(0);
        if pa > pb {
            return true;
        }
        if pa < pb {
            return false;
        }
    }
    false
}
