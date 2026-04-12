use crate::adapters;
use crate::scanner;
use crate::state::{AppState, CrashEvent, SessionStatus, TrackedSession};
use std::collections::HashSet;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

type SharedState = Arc<Mutex<AppState>>;

/// Run the initial scan on startup to detect orphaned sessions
pub async fn initial_scan(state: SharedState, app: tauri::AppHandle) {
    // Check Claude's session files for orphaned PIDs
    if let Some(home) = dirs::home_dir() {
        let sessions_dir = home.join(".claude").join("sessions");
        if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
            let mut orphans = Vec::new();
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(pid) = val.get("pid").and_then(|v| v.as_u64()) {
                                if !scanner::is_pid_alive(pid as u32) {
                                    // This is an orphaned session from a crash
                                    let session_id = val
                                        .get("sessionId")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    let cwd = val
                                        .get("cwd")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("~")
                                        .to_string();

                                    let process = scanner::DetectedProcess {
                                        pid: pid as u32,
                                        tool: "claude".to_string(),
                                        tool_color: "#8B5CF6".to_string(),
                                        comm: "claude".to_string(),
                                        cwd: Some(cwd.clone()),
                                        args: String::new(),
                                    };

                                    let adapter = adapters::get_adapter("claude");
                                    let meta = adapter.resolve_metadata(&process);
                                    let resume_cmd = adapter.resume_command(&process, &meta);
                                    let project_name = std::path::Path::new(&cwd)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();

                                    let id = format!("claude:{}", session_id);
                                    let tracked = TrackedSession {
                                        id: id.clone(),
                                        tool: "claude".to_string(),
                                        tool_color: "#8B5CF6".to_string(),
                                        pid: pid as u32,
                                        cwd,
                                        project_name,
                                        started_at: val
                                            .get("startedAt")
                                            .and_then(|v| v.as_i64())
                                            .unwrap_or(0)
                                            / 1000,
                                        last_seen: chrono::Utc::now().timestamp(),
                                        status: SessionStatus::Crashed,
                                        metadata: meta,
                                        resume_cmd,
                                    };

                                    orphans.push(id.clone());
                                    let mut s = state.lock().await;
                                    s.sessions.insert(id, tracked);
                                }
                            }
                        }
                    }
                }
            }

            if !orphans.is_empty() {
                let mut s = state.lock().await;
                s.crashes.push(CrashEvent {
                    id: format!("startup-{}", chrono::Utc::now().timestamp()),
                    detected_at: chrono::Utc::now().timestamp(),
                    sessions: orphans.clone(),
                    dismissed: false,
                });

                // Send notification
                send_crash_notification(orphans.len());

                // Emit event to frontend
                let _ = app.emit("sessions-updated", ());
                let _ = app.emit("crash-detected", ());

                // Update tray icon
                update_tray_icon(&app, true);
            }
        }
    }
}

/// Main polling loop - runs every 5 seconds
pub async fn start_polling(state: SharedState, app: tauri::AppHandle) {
    let mut interval = time::interval(Duration::from_secs(5));

    loop {
        interval.tick().await;
        poll_once(&state, &app).await;
    }
}

async fn poll_once(state: &SharedState, app: &tauri::AppHandle) {
    // Scan for currently running AI processes
    let processes = scanner::scan_processes();

    let mut s = state.lock().await;
    let now = chrono::Utc::now().timestamp();

    // Track which sessions are still alive
    let mut alive_ids: HashSet<String> = HashSet::new();

    // Process each detected running process
    for process in &processes {
        let adapter = adapters::get_adapter(&process.tool);
        let meta = adapter.resolve_metadata(process);
        let resume_cmd = adapter.resume_command(process, &meta);

        let id = if let Some(ref sid) = meta.session_id {
            format!("{}:{}", process.tool, sid)
        } else {
            format!("{}:{}", process.tool, process.pid)
        };

        alive_ids.insert(id.clone());

        let project_name = process
            .cwd
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        if let Some(existing) = s.sessions.get_mut(&id) {
            // Update last seen and refresh metadata
            existing.last_seen = now;
            existing.status = SessionStatus::Active;
            existing.metadata = meta;
            existing.resume_cmd = resume_cmd;
        } else {
            // New session detected
            let tracked = TrackedSession {
                id: id.clone(),
                tool: process.tool.clone(),
                tool_color: process.tool_color.clone(),
                pid: process.pid,
                cwd: process.cwd.clone().unwrap_or_else(|| "~".to_string()),
                project_name,
                started_at: now,
                last_seen: now,
                status: SessionStatus::Active,
                metadata: meta,
                resume_cmd,
            };
            s.sessions.insert(id, tracked);
        }
    }

    // Check for crashed sessions: previously active but no longer alive
    let mut new_crashes = Vec::new();
    for (id, session) in s.sessions.iter_mut() {
        if session.status == SessionStatus::Active && !alive_ids.contains(id) {
            // Check if PID is truly dead (double-check)
            if !scanner::is_pid_alive(session.pid) {
                session.status = SessionStatus::Crashed;
                new_crashes.push(id.clone());
            }
        }
    }

    // Create crash event if sessions died
    if !new_crashes.is_empty() {
        s.crashes.push(CrashEvent {
            id: format!("crash-{}", now),
            detected_at: now,
            sessions: new_crashes.clone(),
            dismissed: false,
        });

        s.last_updated = now;
        drop(s); // Release lock before notification

        send_crash_notification(new_crashes.len());
        let _ = app.emit("crash-detected", ());
        update_tray_icon(app, true);
    } else {
        // Check if there are any crashes remaining
        let has_crashes = s
            .sessions
            .values()
            .any(|sess| sess.status == SessionStatus::Crashed);
        s.last_updated = now;
        drop(s);

        update_tray_icon(app, has_crashes);
    }

    // Clean up old recovered sessions (older than 1 hour)
    let mut s = state.lock().await;
    let cutoff = now - 3600;
    s.sessions
        .retain(|_, sess| sess.status != SessionStatus::Recovered || sess.last_seen > cutoff);

    let _ = app.emit("sessions-updated", ());
}

fn send_crash_notification(count: usize) {
    let title = format!(
        "uhoh! {} session{} crashed",
        count,
        if count > 1 { "s" } else { "" }
    );
    let body = "Click the menu bar icon to recover.";

    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            &format!(
                "display notification \"{}\" with title \"{}\" sound name \"Basso\"",
                body, title
            ),
        ])
        .spawn();
}

fn update_tray_icon(app: &tauri::AppHandle, has_crashes: bool) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        let icon_bytes = if has_crashes {
            include_bytes!("../icons/tray-crash.png").as_slice()
        } else {
            include_bytes!("../icons/tray-ok.png").as_slice()
        };
        if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
            let _ = tray.set_icon(Some(icon));
        }
    }
}
