mod adapters;
mod coordinator;
mod fs_watcher;
mod kqueue_watcher;
mod libproc_scanner;
mod recovery;
mod state;
mod store;

use state::{AppState, SessionStatus};
use std::sync::Arc;
use tauri::{
    image::Image,
    tray::TrayIconBuilder,
    Manager,
};
use tokio::sync::Mutex;

type SharedState = Arc<Mutex<AppState>>;

#[tauri::command]
async fn get_sessions(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<state::TrackedSession>, String> {
    let s = state.lock().await;
    Ok(s.sessions.values().cloned().collect())
}

#[tauri::command]
async fn get_crashes(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<state::CrashEvent>, String> {
    let s = state.lock().await;
    Ok(s.crashes.clone())
}

#[tauri::command]
async fn restore_session(session_id: String, tool: String, state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.lock().await;
    if let Some(session) = s.sessions.get_mut(&session_id) {
        recovery::restore_one(session, &tool).map_err(|e| e.to_string())?;
        session.status = SessionStatus::Recovered;
    }
    Ok(())
}

#[tauri::command]
async fn restore_all(tool: String, state: tauri::State<'_, SharedState>) -> Result<u32, String> {
    let mut s = state.lock().await;
    let crashed: Vec<String> = s
        .sessions
        .iter()
        .filter(|(_, sess)| sess.status == SessionStatus::Crashed)
        .map(|(id, _)| id.clone())
        .collect();

    let count = crashed.len() as u32;
    for id in &crashed {
        if let Some(session) = s.sessions.get_mut(id) {
            recovery::restore_one(session, &tool).map_err(|e| e.to_string())?;
            session.status = SessionStatus::Recovered;
        }
    }

    let still_crashed: std::collections::HashSet<String> = s
        .sessions
        .iter()
        .filter(|(_, sess)| sess.status == SessionStatus::Crashed)
        .map(|(id, _)| id.clone())
        .collect();

    s.crashes.retain(|c| {
        c.sessions.iter().any(|sid| still_crashed.contains(sid))
    });

    Ok(count)
}

#[tauri::command]
async fn dismiss_crash(crash_id: String, state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let mut s = state.lock().await;
    s.crashes.retain(|c| c.id != crash_id);
    for session in s.sessions.values_mut() {
        if session.status == SessionStatus::Crashed {
            session.status = SessionStatus::Recovered;
        }
    }
    Ok(())
}

#[tauri::command]
async fn open_multiple(session_ids: Vec<String>, tool: String, state: tauri::State<'_, SharedState>) -> Result<u32, String> {
    let mut s = state.lock().await;

    // For tmux, use the grid layout that opens all sessions in split panes
    if tool == "tmux" {
        let sessions: Vec<&state::TrackedSession> = session_ids
            .iter()
            .filter_map(|id| s.sessions.get(id))
            .collect();
        recovery::restore_grid(&sessions).map_err(|e| e.to_string())?;
        let count = sessions.len() as u32;
        for id in &session_ids {
            if let Some(session) = s.sessions.get_mut(id) {
                session.status = SessionStatus::Recovered;
            }
        }
        return Ok(count);
    }

    // Collect commands, then open as tabs
    let cmds: Vec<String> = session_ids
        .iter()
        .filter_map(|id| s.sessions.get(id))
        .map(|sess| sess.resume_cmd.clone())
        .collect();

    let count = cmds.len() as u32;
    recovery::restore_batch(&cmds, &tool).map_err(|e| e.to_string())?;

    for id in &session_ids {
        if let Some(session) = s.sessions.get_mut(id) {
            session.status = SessionStatus::Recovered;
        }
    }
    Ok(count)
}

#[tauri::command]
fn get_available_tools() -> Vec<recovery::TerminalTool> {
    recovery::detect_tools()
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

pub fn run() {
    let app_state: SharedState = Arc::new(Mutex::new(AppState::new()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state.clone())
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            get_crashes,
            restore_session,
            restore_all,
            dismiss_crash,
            open_multiple,
            get_available_tools,
            quit_app,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                // Apply macOS vibrancy (frosted glass)
                #[cfg(target_os = "macos")]
                {
                    use window_vibrancy::{apply_vibrancy, NSVisualEffectMaterial};
                    let _ = apply_vibrancy(&window, NSVisualEffectMaterial::HudWindow, None, Some(12.0));
                }

                // Auto-hide popup on focus loss
                let win_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(false) = event {
                        let _ = win_clone.hide();
                    }
                });
            }

            let icon = match Image::from_bytes(include_bytes!("../icons/tray-ok.png")) {
                Ok(img) => img,
                Err(e) => {
                    eprintln!("uhoh: failed to load tray icon: {}", e);
                    return Ok(());
                }
            };

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .icon_as_template(true)
                .tooltip("uhoh - AI Session Monitor")
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                // rect.position/size are logical coordinates
                                let scale = window.scale_factor().unwrap_or(2.0);
                                let icon_pos = rect.position.to_logical::<f64>(scale);
                                let icon_size = rect.size.to_logical::<f64>(scale);

                                let window_width = 320.0_f64;
                                let icon_center_x = icon_pos.x + (icon_size.width / 2.0);
                                let x = icon_center_x - (window_width / 2.0);
                                let y = icon_pos.y + icon_size.height;

                                let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Start the three-layer coordinator
            let coord_state = app_state.clone();
            let coord_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                coordinator::Coordinator::start(coord_state, coord_handle).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running uhoh");
}
