//! Unsigned builds have a changing code identity. Reset only our two capture
//! permissions once per installed version, before any permission prompt.
use log::{info, warn};
use std::{fs, io, path::Path, process::Command};
use tauri::api::dialog::{MessageDialogBuilder, MessageDialogButtons};

const SERVICES: [&str; 2] = ["ScreenCapture", "Accessibility"];

fn reset(app: &tauri::AppHandle, service: &str) -> bool {
    match Command::new("/usr/bin/tccutil")
        .args(["reset", service, &app.config().tauri.bundle.identifier])
        .output()
    {
        Ok(output) if output.status.success() => {
            info!("Reset Snapshoot {service} permission");
            true
        }
        result => {
            warn!("Could not reset Snapshoot {service}: {result:?}");
            false
        }
    }
}

fn chinese() -> bool {
    crate::config::get("app_language")
        .and_then(|value| value.as_str().map(|s| s.starts_with("zh")))
        .unwrap_or(false)
}

fn open_settings(pane: &str) {
    if let Err(err) = Command::new("/usr/bin/open")
        .arg(format!(
            "x-apple.systempreferences:com.apple.preference.security?{pane}"
        ))
        .spawn()
    {
        warn!("Cannot open Privacy & Security: {err}");
    }
}

pub fn show_status(app: tauri::AppHandle, index: usize) {
    let zh = chinese();
    let allowed = if index == 0 {
        crate::capture_macos::screen_recording_allowed()
    } else {
        macos_accessibility_client::accessibility::application_is_trusted()
    };
    let name = if zh {
        ["屏幕录制", "辅助功能"][index]
    } else {
        ["Screen Recording", "Accessibility"][index]
    };
    let status = match (zh, allowed) {
        (true, true) => "已授权",
        (true, false) => "未授权",
        (false, true) => "Granted",
        (false, false) => "Not granted",
    };
    let action = if allowed {
        if zh {
            "打开系统设置"
        } else {
            "Open System Settings"
        }
    } else if zh {
        "重置并打开设置"
    } else {
        "Reset and open Settings"
    };
    let next = if index == 0 {
        if zh {
            "检查辅助功能"
        } else {
            "Check Accessibility"
        }
    } else if zh {
        "关闭"
    } else {
        "Close"
    };
    let pane = ["Privacy_ScreenCapture", "Privacy_Accessibility"][index];
    MessageDialogBuilder::new(name, status)
        .buttons(MessageDialogButtons::OkCancelWithLabels(action.into(), next.into()))
        .show(move |proceed| {
            if proceed {
                if !allowed && !reset(&app, SERVICES[index]) {
                    MessageDialogBuilder::new(name, if zh {
                        "自动重置失败。请在系统设置中移除旧条目，再添加 Snapshoot。授权后请退出并重新打开应用。"
                    } else {
                        "Reset failed. Remove the old entry in System Settings, add Snapshoot again, then quit and reopen it."
                    }).show(move |_| open_settings(pane));
                } else {
                    open_settings(pane);
                }
            } else if index == 0 {
                show_status(app, 1);
            }
        });
}

fn build_identity(version: &str) -> String {
    let hash = std::env::current_exe().ok().and_then(|exe| {
        let output = Command::new("/usr/bin/codesign")
            .args(["-d", "--verbose=4"])
            .arg(exe)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .find_map(|line| line.strip_prefix("CDHash=").map(str::to_owned))
    });
    hash.map_or_else(|| version.to_owned(), |hash| format!("{version} {hash}"))
}

fn needs_reset(previous: Option<&str>, current: &str, existing_install: bool) -> bool {
    match previous {
        Some(version) => version != current,
        None => existing_install, // Migration from versions without this marker.
    }
}

fn record_version(path: &Path, version: &str) -> io::Result<()> {
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("Missing parent"))?,
    )?;
    let pending = path.with_extension("pending");
    fs::write(&pending, version)?;
    fs::rename(pending, path)
}

/// Returns true when upgrade guidance replaces the normal first-run prompt.
pub fn prepare(app: &tauri::AppHandle, existing_install: bool) -> bool {
    // Development/smoke runs must never invalidate the installed app's grants.
    if cfg!(debug_assertions) || crate::smoke::enabled() {
        return false;
    }
    let Some(dir) = app.path_resolver().app_config_dir() else {
        warn!("Cannot locate permission migration marker; skipping reset");
        return false;
    };
    let marker = dir.join("macos-permissions-version");
    let previous = match fs::read_to_string(&marker) {
        Ok(value) => Some(value),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => {
            warn!("Cannot read permission migration marker: {err}; skipping reset");
            return false;
        }
    };
    let version = build_identity(&app.package_info().version.to_string());
    if previous.as_deref() == Some(version.as_str()) {
        return false;
    }
    let should_reset = needs_reset(previous.as_deref(), &version, existing_install);
    // Persist BEFORE resetting. Even a failed reset must not cause a reset loop
    // that removes freshly granted access on the user's next launch.
    if let Err(err) = record_version(&marker, &version) {
        warn!("Cannot save permission migration marker: {err}; skipping reset");
        return false;
    }
    if !should_reset {
        return false;
    }
    let failed: Vec<_> = SERVICES
        .iter()
        .copied()
        .filter(|service| !reset(app, service))
        .collect();
    let chinese = chinese();
    let failed_names = failed
        .iter()
        .map(|service| match (chinese, *service) {
            (true, "ScreenCapture") => "屏幕录制",
            (true, _) => "辅助功能",
            (false, "ScreenCapture") => "Screen Recording",
            (false, _) => "Accessibility",
        })
        .collect::<Vec<_>>()
        .join(if chinese { "、" } else { ", " });
    let (title, message, open, later) = if chinese {
        let status = if failed.is_empty() {
            "已清除 Snapshoot 旧版的屏幕录制和辅助功能授权，请重新授权。"
        } else {
            "部分旧权限未能自动清除。请在系统设置中移除旧的 Snapshoot 条目，再添加“应用程序”里的 Snapshoot。"
        };
        let details = if failed.is_empty() {
            String::new()
        } else {
            format!("\n未能重置：{failed_names}。可从菜单栏的“权限检查与修复”重试。")
        };
        ("更新后重新授权", format!("{status}{details}\n\n在“隐私与安全性”中分别打开“屏幕录制”（或“屏幕与系统音频录制”）和“辅助功能”，允许 Snapshoot。完成后请完全退出并重新打开应用。\n\n本版本不会在下次启动时重复清除权限。"), "打开系统设置", "稍后处理")
    } else {
        let status = if failed.is_empty() {
            "Snapshoot's previous Screen Recording and Accessibility permissions were reset. Please grant them again."
        } else {
            "Some permissions could not be reset. Remove the old Snapshoot entries in System Settings, then add Snapshoot from Applications again."
        };
        let details = if failed.is_empty() {
            String::new()
        } else {
            format!("\nReset failed for {failed_names}. Retry from Check and repair permissions in the menu bar.")
        };
        ("Permissions after update", format!("{status}{details}\n\nIn Privacy & Security, enable Snapshoot under Screen Recording (or Screen & System Audio Recording) and Accessibility. Then fully quit and reopen Snapshoot.\n\nThis version will not reset permissions again on the next launch."), "Open System Settings", "Later")
    };
    MessageDialogBuilder::new(title, message)
        .buttons(MessageDialogButtons::OkCancelWithLabels(
            open.into(),
            later.into(),
        ))
        .show(|open| {
            if open {
                if let Err(err) = Command::new("/usr/bin/open")
                    .arg("x-apple.systempreferences:com.apple.preference.security?Privacy")
                    .spawn()
                {
                    warn!("Cannot open Privacy & Security: {err}");
                }
            }
        });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resets_only_on_version_change_or_legacy_migration() {
        assert!(!needs_reset(None, "1.1.1", false));
        assert!(needs_reset(None, "1.1.1", true));
        assert!(!needs_reset(Some("1.1.1"), "1.1.1", true));
        assert!(needs_reset(Some("1.1.0"), "1.1.1", true));
        // Returning to a previously installed version also changes code identity.
        assert!(needs_reset(Some("1.2.0"), "1.1.1", true));
    }

    #[test]
    fn persisted_attempt_prevents_resetting_fresh_grants_on_restart() {
        let dir =
            std::env::temp_dir().join(format!("snapshoot-permission-test-{}", std::process::id()));
        let marker = dir.join("macos-permissions-version");
        record_version(&marker, "1.1.0").unwrap();
        let old = fs::read_to_string(&marker).unwrap();
        assert!(needs_reset(Some(&old), "1.1.1", true));
        // Persist even if the subsequent OS reset fails or only partly succeeds.
        record_version(&marker, "1.1.1").unwrap();
        let attempted = fs::read_to_string(&marker).unwrap();
        assert!(!needs_reset(Some(&attempted), "1.1.1", true));
        assert!(!marker.with_extension("pending").exists());
        // A failed save must be reported so the caller can skip permission reset.
        assert!(record_version(&marker.join("invalid-child"), "1.1.2").is_err());
        fs::remove_dir_all(dir).unwrap();
    }
}
