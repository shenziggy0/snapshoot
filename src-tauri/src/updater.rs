use crate::config::get;
use crate::window::updater_window;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Snapshoot ships unsigned installers and has no updater signing key, so
/// Tauri's built-in updater is not used. Instead the latest GitHub release is
/// compared with the running version and the user downloads the installer.
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/shenziggy0/snapshoot/releases/latest";

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    assets: Vec<GithubAsset>,
}

#[derive(Serialize)]
pub struct ReleaseInfo {
    current: String,
    latest: String,
    has_update: bool,
    notes: String,
    download_url: Option<String>,
    page_url: String,
}

/// "v1.2.3" / "1.2.3-beta" -> [1, 2, 3]. Missing or non-numeric parts count as 0.
fn version_parts(version: &str) -> [u64; 3] {
    let core = version.trim().trim_start_matches('v');
    let core = core.split(['-', '+']).next().unwrap_or("");
    let mut parts = [0; 3];
    for (slot, part) in parts.iter_mut().zip(core.split('.')) {
        *slot = part.parse().unwrap_or(0);
    }
    parts
}

fn is_newer(latest: &str, current: &str) -> bool {
    version_parts(latest) > version_parts(current)
}

/// Pick the installer built by package.yml for this platform.
fn asset_matches(name: &str) -> bool {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        name.ends_with("_aarch64.dmg")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        name.ends_with("_x64.dmg")
    } else if cfg!(all(windows, target_arch = "x86_64")) {
        name.ends_with("_x64-setup.exe")
    } else {
        false
    }
}

async fn fetch_latest_release() -> Result<GithubRelease, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        // GitHub's API rejects requests without a User-Agent.
        .user_agent(concat!("Snapshoot/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| err.to_string())?;
    let response = client
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("GitHub API returned {status}"));
    }
    response.json().await.map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn check_latest_release(app_handle: tauri::AppHandle) -> Result<ReleaseInfo, String> {
    let current = app_handle.package_info().version.to_string();
    let release = fetch_latest_release().await?;
    let latest = release.tag_name.trim_start_matches('v').to_string();
    let download_url = release
        .assets
        .into_iter()
        .find(|asset| asset_matches(&asset.name))
        .map(|asset| asset.browser_download_url);
    Ok(ReleaseInfo {
        has_update: is_newer(&latest, &current),
        current,
        latest,
        notes: release.body.unwrap_or_default(),
        download_url,
        page_url: release.html_url,
    })
}

/// Startup check. Network failures are only logged; the updater window opens
/// only when a newer release exists.
pub fn check_update(app_handle: tauri::AppHandle) {
    let enabled = get("check_update").and_then(|v| v.as_bool()).unwrap_or(true);
    if !enabled || crate::smoke::enabled() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        match check_latest_release(app_handle).await {
            Ok(release) if release.has_update => {
                info!("New version available: {} -> {}", release.current, release.latest);
                updater_window();
            }
            Ok(_) => {}
            Err(err) => warn!("Check update failed: {err}"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_is_detected_per_component() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("1.0.0", "1.0.0"));
        assert!(!is_newer("v1.0.0", "1.0.1"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }
}
