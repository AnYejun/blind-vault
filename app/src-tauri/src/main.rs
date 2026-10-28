// Blind Vault — menu-bar vault for secrets an AI agent can use but never see.
// Shares ~/.blindvault/manifest.json and the `blindvault.*` Keychain namespace
// with the CLI, so keys added here are immediately visible to `vault ls`.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

fn vault_dir() -> PathBuf {
    std::env::var("BLINDVAULT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".blindvault"))
}

fn manifest_path() -> PathBuf {
    vault_dir().join("manifest.json")
}

fn load_manifest() -> Value {
    std::fs::read_to_string(manifest_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({ "secrets": [] }))
}

fn save_manifest(v: &Value) -> Result<(), String> {
    std::fs::create_dir_all(vault_dir()).map_err(|e| e.to_string())?;
    std::fs::write(manifest_path(), serde_json::to_string_pretty(v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn valid_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || ".-_".contains(c))
}

fn today() -> String {
    Command::new("date")
        .arg("+%F")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[tauri::command]
fn list_secrets() -> Value {
    load_manifest()
}

fn pbcopy(text: &str) -> Result<(), String> {
    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .ok_or("pbcopy stdin unavailable")?
        .write_all(text.as_bytes())
        .map_err(|e| e.to_string())?;
    child.wait().ok();
    Ok(())
}

#[tauri::command]
fn add_secret(
    name: String,
    value: String,
    service: String,
    env: String,
    allow: String,
    note: String,
    account: String,
) -> Result<String, String> {
    if !valid_name(&name) {
        return Err("name must be letters, digits, dot, dash, underscore".into());
    }
    if value.is_empty() {
        return Err("empty value — nothing stored".into());
    }
    let account = if account.trim().is_empty() {
        std::env::var("USER").unwrap_or_else(|_| "unknown".into())
    } else {
        account.trim().to_string()
    };
    let envvar = if env.trim().is_empty() {
        name.to_uppercase().replace(['-', '.'], "_")
    } else {
        env.trim().to_string()
    };
    let status = Command::new("security")
        .args([
            "add-generic-password",
            "-U",
            "-a",
            &account,
            "-s",
            &format!("blindvault.{name}"),
            "-j",
            "managed by blind-vault",
            "-w",
            &value,
        ])
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("keychain write failed".into());
    }

    let mut m = load_manifest();
    let secrets = m["secrets"].as_array_mut().ok_or("bad manifest")?;
    secrets.retain(|s| s["name"] != name.as_str());
    secrets.push(json!({
        "name": name,
        "service": service.trim(),
        "account": account,
        "env": envvar,
        "allowed_for": allow.split(',').map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>(),
        "note": note.trim(),
        "created": today(),
        "last_used": Value::Null,
    }));
    secrets.sort_by(|a, b| {
        a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or(""))
    });
    save_manifest(&m)?;
    Ok(format!("Stored '{name}'. The value never left this machine."))
}

#[tauri::command]
fn remove_secret(name: String) -> Result<String, String> {
    if !valid_name(&name) {
        return Err("bad name".into());
    }
    Command::new("security")
        .args(["delete-generic-password", "-s", &format!("blindvault.{name}")])
        .output()
        .ok();
    let mut m = load_manifest();
    if let Some(secrets) = m["secrets"].as_array_mut() {
        secrets.retain(|s| s["name"] != name.as_str());
    }
    save_manifest(&m)?;
    Ok(format!("removed '{name}' (Keychain + manifest)"))
}

#[tauri::command]
fn copy_secret(name: String) -> Result<String, String> {
    if !valid_name(&name) {
        return Err("bad name".into());
    }
    let out = Command::new("security")
        .args(["find-generic-password", "-w", "-s", &format!("blindvault.{name}")])
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("keychain lookup failed for '{name}'"));
    }
    let value = String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string();
    pbcopy(&value)?;
    drop(value);
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(30));
        if let Ok(mut c) = Command::new("pbcopy").stdin(Stdio::piped()).spawn() {
            drop(c.stdin.take());
            c.wait().ok();
        }
    });
    Ok(format!("'{name}' on clipboard — clears in 30 s"))
}

// The account/ID is pointer metadata (not a secret) — copying it is free.
#[tauri::command]
fn copy_account(name: String) -> Result<String, String> {
    let m = load_manifest();
    let acct = m["secrets"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["name"] == name.as_str()))
        .and_then(|s| s["account"].as_str())
        .unwrap_or("")
        .to_string();
    if acct.is_empty() {
        return Err("no account stored".into());
    }
    pbcopy(&acct)?;
    Ok(format!("ID '{acct}' copied"))
}

fn toggle(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            w.hide().ok();
        } else {
            w.show().ok();
            w.set_focus().ok();
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let win = app.get_webview_window("main").expect("main window");
            // Follow the system appearance: the UI ships light + dark tokens.
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial, NSVisualEffectState};
                apply_vibrancy(
                    &win,
                    NSVisualEffectMaterial::HudWindow,
                    Some(NSVisualEffectState::Active),
                    Some(16.0),
                )
                .ok();
            }

            // ⌥⌘V toggles the window from anywhere, Raycast-style.
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                app.global_shortcut()
                    .on_shortcut("alt+cmd+v", |app, _shortcut, event| {
                        if event.state() == ShortcutState::Pressed {
                            toggle(app);
                        }
                    })
                    .ok();
            }

            let open = MenuItem::with_id(app, "open", "Open Blind Vault  ⌥⌘V", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let tray_icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
            TrayIconBuilder::with_id("blind-vault-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => toggle(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle(tray.app_handle());
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|win, ev| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = ev {
                win.hide().ok();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            list_secrets,
            add_secret,
            remove_secret,
            copy_secret,
            copy_account
        ])
        .build(tauri::generate_context!())
        .expect("error while running blind-vault")
        .run(|app, event| {
            // Relaunching the app (Finder/Spotlight) summons the window.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = event {
                if let Some(w) = app.get_webview_window("main") {
                    w.show().ok();
                    w.set_focus().ok();
                }
            }
            let _ = (app, &event);
        });
}
