use crate::adapters;
use crate::fs_watcher::{self, FsEvent};
use crate::kqueue_watcher::ProcessEvent;
#[cfg(target_os = "macos")]
use crate::kqueue_watcher::KqueueWatcher;
use crate::libproc_scanner;
use crate::state::{AppState, CrashEvent, SessionStatus, TrackedSession};
use crate::store;
use notify::RecommendedWatcher;
use std::collections::HashSet;
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::Mutex;
use tokio::time::{self, Duration};

type SharedState = Arc<Mutex<AppState>>;

pub struct Coordinator {
    state: SharedState,
    app_handle: tauri::AppHandle,
    #[cfg(target_os = "macos")]
    kqueue: KqueueWatcher,
    _fs_watcher: Option<RecommendedWatcher>, // Must keep alive
}

impl Coordinator {
    pub async fn start(state: SharedState, app_handle: tauri::AppHandle) {
        // Layer 1: kqueue process watcher (macOS only)
        #[cfg(target_os = "macos")]
        let (kqueue, mut process_rx) = KqueueWatcher::new();
        // On non-macOS, create a channel that never receives (polling handles everything)
        #[cfg(not(target_os = "macos"))]
        let (_process_tx, mut process_rx) = tokio::sync::mpsc::unbounded_channel::<ProcessEvent>();

        // Layer 2: FSEvents/inotify file watcher
        let (mut fs_rx, fs_watcher) = fs_watcher::start_watching();

        let coordinator = Coordinator {
            state: state.clone(),
            app_handle: app_handle.clone(),
            #[cfg(target_os = "macos")]
            kqueue,
            _fs_watcher: fs_watcher,
        };

        // Initial scan: discover running sessions and orphaned session files
        coordinator.initial_scan().await;

        // Scan interval: 10s on macOS (kqueue handles instant detection), 5s on other platforms
        #[cfg(target_os = "macos")]
        let scan_secs = 10;
        #[cfg(not(target_os = "macos"))]
        let scan_secs = 5;

        let mut scan_interval = time::interval(Duration::from_secs(scan_secs));
        let mut save_interval = time::interval(Duration::from_secs(10));

        loop {
            tokio::select! {
                // Layer 1: Instant process death (kqueue on macOS, never fires on other platforms)
                Some(event) = process_rx.recv() => {
                    coordinator.handle_process_event(event).await;
                }

                // Layer 2: File system changes
                Some(event) = fs_rx.recv() => {
                    coordinator.handle_fs_event(event).await;
                }

                // Layer 3: Periodic full scan (fallback)
                _ = scan_interval.tick() => {
                    coordinator.full_scan().await;
                }

                // Periodic state save
                _ = save_interval.tick() => {
                    let s = coordinator.state.lock().await;
                    let _ = store::save_state(&s);
                }
            }
        }
    }

    /// Initial scan: find all running AI sessions + orphaned session files
    async fn initial_scan(&self) {
        // Load persisted state if available
        if let Some(saved) = store::load_state() {
            let mut s = self.state.lock().await;
            for crash in saved.crashes {
                if !crash.dismissed {
                    s.crashes.push(crash);
                }
            }
        }

        // Scan running processes
        let processes = libproc_scanner::scan_processes();
        let mut s = self.state.lock().await;
        let now = chrono::Utc::now().timestamp();

        for process in &processes {
            let adapter = adapters::get_adapter(&process.tool);
            let meta = adapter.resolve_metadata(&crate::libproc_scanner::DetectedProcess {
                pid: process.pid,
                tool: process.tool.clone(),
                tool_color: process.tool_color.clone(),
                comm: process.comm.clone(),
                cwd: process.cwd.clone(),
                args: process.args.clone(),
                start_time: process.start_time,
            });
            let resume_cmd = adapter.resume_command(&crate::libproc_scanner::DetectedProcess {
                pid: process.pid,
                tool: process.tool.clone(),
                tool_color: process.tool_color.clone(),
                comm: process.comm.clone(),
                cwd: process.cwd.clone(),
                args: process.args.clone(),
                start_time: process.start_time,
            }, &meta);

            let id = if let Some(ref sid) = meta.session_id {
                format!("{}:{}", process.tool, sid)
            } else {
                format!("{}:{}", process.tool, process.pid)
            };

            let project_name = process
                .cwd
                .as_ref()
                .and_then(|p| std::path::Path::new(p).file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let tracked = TrackedSession {
                id: id.clone(),
                tool: process.tool.clone(),
                tool_color: process.tool_color.clone(),
                pid: process.pid,
                cwd: process.cwd.clone().unwrap_or_else(|| "~".to_string()),
                project_name,
                started_at: process.start_time as i64,
                last_seen: now,
                status: SessionStatus::Active,
                metadata: meta,
                resume_cmd,
                start_time: process.start_time,
                from_index: false,
            };

            s.sessions.insert(id, tracked);

            // Register this PID with kqueue for instant death detection (macOS only)
            #[cfg(target_os = "macos")]
            self.kqueue.watch_pid(process.pid);
        }

        // Check Claude's session files for orphaned PIDs
        self.scan_orphaned_session_files(&mut s, now).await;

        // Load recent sessions from tool session indices
        self.load_recent_claude_sessions(&mut s, now);
        Self::load_codex_sessions(&mut s);

        let has_crashes = s.sessions.values().any(|sess| sess.status == SessionStatus::Crashed);
        drop(s);

        if has_crashes {
            send_crash_notification(&self.app_handle);
            update_tray_icon(&self.app_handle, true);
        }

        let _ = self.app_handle.emit("sessions-updated", ());
    }

    /// Scan Claude's session files for orphaned PIDs (crash detection at startup)
    async fn scan_orphaned_session_files(&self, s: &mut AppState, now: i64) {
        let Some(home) = dirs::home_dir() else { return };
        let sessions_dir = home.join(".claude").join("sessions");
        let Ok(entries) = std::fs::read_dir(&sessions_dir) else { return };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let Ok(content) = std::fs::read_to_string(&path) else { continue };
            let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) else { continue };
            let Some(pid) = val.get("pid").and_then(|v| v.as_u64()) else { continue };

            if libproc_scanner::is_pid_alive(pid as u32) {
                continue;
            }

            let session_id = val.get("sessionId").and_then(|v| v.as_str()).unwrap_or("unknown");
            let id = format!("claude:{}", session_id);

            if s.sessions.contains_key(&id) {
                continue;
            }

            let cwd = val.get("cwd").and_then(|v| v.as_str()).unwrap_or("~").to_string();
            let started_at = val.get("startedAt").and_then(|v| v.as_i64()).unwrap_or(0) / 1000;

            let process = crate::libproc_scanner::DetectedProcess {
                pid: pid as u32,
                tool: "claude".to_string(),
                tool_color: "#8B5CF6".to_string(),
                comm: "claude".to_string(),
                cwd: Some(cwd.clone()),
                args: String::new(),
                start_time: started_at as u64,
            };

            let adapter = adapters::get_adapter("claude");
            let meta = adapter.resolve_metadata(&process);
            let resume_cmd = adapter.resume_command(&process, &meta);
            let project_name = std::path::Path::new(&cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let tracked = TrackedSession {
                id: id.clone(),
                tool: "claude".to_string(),
                tool_color: "#8B5CF6".to_string(),
                pid: pid as u32,
                cwd,
                project_name,
                started_at,
                last_seen: now,
                status: SessionStatus::Ended,
                metadata: meta,
                resume_cmd,
                start_time: started_at as u64,
                from_index: false,
            };

            s.sessions.insert(id, tracked);
        }
    }

    /// Load recent sessions (last 24h) from Claude's sessions-index.json files
    fn load_recent_claude_sessions(&self, s: &mut AppState, now: i64) {
        let Some(home) = dirs::home_dir() else { return };
        let projects_dir = home.join(".claude").join("projects");
        let Ok(projects) = std::fs::read_dir(&projects_dir) else { return };
        let cutoff = now - 86400;

        for project_entry in projects.flatten() {
            let index_path = project_entry.path().join("sessions-index.json");
            let Ok(content) = std::fs::read_to_string(&index_path) else { continue };
            let Ok(index) = serde_json::from_str::<serde_json::Value>(&content) else { continue };
            let Some(entries) = index.get("entries").and_then(|e| e.as_array()) else { continue };

            for entry in entries {
                let Some(session_id) = entry.get("sessionId").and_then(|v| v.as_str()) else { continue };
                let id = format!("claude:{}", session_id);

                if s.sessions.contains_key(&id) { continue; }

                let modified_str = entry.get("modified").and_then(|v| v.as_str()).unwrap_or("");
                let modified_ts = chrono::DateTime::parse_from_rfc3339(modified_str)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);

                if modified_ts < cutoff { continue; }

                let summary = entry.get("summary").and_then(|v| v.as_str()).map(|s| s.to_string());
                let first_prompt = entry.get("firstPrompt").and_then(|v| v.as_str()).map(|s| s.to_string());
                let message_count = entry.get("messageCount").and_then(|v| v.as_u64()).map(|n| n as u32);
                let git_branch = entry.get("gitBranch").and_then(|v| v.as_str()).map(|s| s.to_string());
                let project_path = entry.get("projectPath").and_then(|v| v.as_str()).unwrap_or("~");

                let project_name = std::path::Path::new(project_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let resume_cmd = format!("cd {} && claude --resume {}", project_path, session_id);

                let created_str = entry.get("created").and_then(|v| v.as_str()).unwrap_or("");
                let created_ts = chrono::DateTime::parse_from_rfc3339(created_str)
                    .map(|dt| dt.timestamp())
                    .unwrap_or(0);

                let tracked = TrackedSession {
                    id: id.clone(),
                    tool: "claude".to_string(),
                    tool_color: "#8B5CF6".to_string(),
                    pid: 0,
                    cwd: project_path.to_string(),
                    project_name,
                    started_at: created_ts,
                    last_seen: modified_ts,
                    status: SessionStatus::Ended,
                    metadata: crate::state::SessionMeta {
                        summary,
                        first_prompt,
                        message_count,
                        git_branch,
                        session_id: Some(session_id.to_string()),
                        session_name: None,
                    },
                    resume_cmd,
                    start_time: created_ts as u64,
                    from_index: true,
                };

                s.sessions.insert(id, tracked);
            }
        }
    }

    /// Load Codex sessions from ~/.codex/session_index.jsonl
    fn load_codex_sessions(s: &mut AppState) {
        let Some(home) = dirs::home_dir() else { return };
        let index_path = home.join(".codex").join("session_index.jsonl");
        let Ok(content) = std::fs::read_to_string(&index_path) else { return };

        for line in content.lines() {
            let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let Some(session_id) = val.get("id").and_then(|v| v.as_str()) else { continue };
            let id = format!("codex:{}", session_id);

            if s.sessions.contains_key(&id) { continue; }

            let thread_name = val.get("thread_name").and_then(|v| v.as_str()).unwrap_or("Untitled");
            let updated = val.get("updated_at").and_then(|v| v.as_str()).unwrap_or("");
            let modified_ts = chrono::DateTime::parse_from_rfc3339(updated)
                .map(|dt| dt.timestamp())
                .unwrap_or(0);

            let resume_cmd = format!("codex resume {}", session_id);

            let tracked = TrackedSession {
                id: id.clone(),
                tool: "codex".to_string(),
                tool_color: "#10A37F".to_string(),
                pid: 0,
                cwd: "~".to_string(),
                project_name: thread_name.to_string(),
                started_at: modified_ts,
                last_seen: modified_ts,
                status: SessionStatus::Ended,
                metadata: crate::state::SessionMeta {
                    summary: Some(thread_name.to_string()),
                    first_prompt: None,
                    message_count: None,
                    git_branch: None,
                    session_id: Some(session_id.to_string()),
                    session_name: Some(thread_name.to_string()),
                },
                resume_cmd,
                start_time: modified_ts as u64,
                from_index: true,
            };

            s.sessions.insert(id, tracked);
        }
    }

    /// Handle process exit event from kqueue (Layer 1)
    async fn handle_process_event(&self, event: ProcessEvent) {
        match event {
            ProcessEvent::Exited { pid } => {
                let mut s = self.state.lock().await;
                let now = chrono::Utc::now().timestamp();

                let session_id = s
                    .sessions
                    .iter()
                    .find(|(_, sess)| sess.pid == pid && sess.status == SessionStatus::Active)
                    .map(|(id, _)| id.clone());

                if let Some(id) = session_id {
                    s.sessions.get_mut(&id).unwrap().status = SessionStatus::Ended;
                    s.sessions.get_mut(&id).unwrap().last_seen = now;

                    if let Some(session) = s.sessions.get(&id) {
                        if session.tool == "claude" {
                            if let Some(ref sid) = session.metadata.session_id {
                                let cwd = session.cwd.clone();
                                let resume_cmd = format!("cd {} && claude --resume {}", cwd, sid);
                                s.sessions.get_mut(&id).unwrap().resume_cmd = resume_cmd;
                            }
                        }
                    }

                    s.last_updated = now;
                    drop(s);
                    let _ = self.app_handle.emit("sessions-updated", ());
                }
            }
        }
    }

    /// Check if multiple sessions died in a short window (bulk crash detection)
    fn detect_bulk_crash(s: &mut AppState, dead_ids: &[String], now: i64) {
        if dead_ids.len() >= 3 {
            for id in dead_ids {
                if let Some(session) = s.sessions.get_mut(id) {
                    session.status = SessionStatus::Crashed;
                }
            }
            s.crashes.push(CrashEvent {
                id: format!("bulk-crash-{}", now),
                detected_at: now,
                sessions: dead_ids.to_vec(),
                dismissed: false,
            });
        }
    }

    /// Handle file system event (Layer 2)
    async fn handle_fs_event(&self, event: FsEvent) {
        match event {
            FsEvent::SessionFileCreated { tool, path } => {
                if tool == "claude" {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                            if let Some(pid) = val.get("pid").and_then(|v| v.as_u64()) {
                                let pid = pid as u32;
                                if libproc_scanner::is_pid_alive(pid) {
                                    #[cfg(target_os = "macos")]
                                    self.kqueue.watch_pid(pid);

                                    let cwd = val.get("cwd").and_then(|v| v.as_str()).unwrap_or("~");
                                    let session_id = val.get("sessionId").and_then(|v| v.as_str()).unwrap_or("unknown");

                                    let start_time = libproc_scanner::get_pid_start_time(pid).unwrap_or(0);
                                    let process = crate::libproc_scanner::DetectedProcess {
                                        pid,
                                        tool: "claude".to_string(),
                                        tool_color: "#8B5CF6".to_string(),
                                        comm: "claude".to_string(),
                                        cwd: Some(cwd.to_string()),
                                        args: String::new(),
                                        start_time,
                                    };

                                    let adapter = adapters::get_adapter("claude");
                                    let meta = adapter.resolve_metadata(&process);
                                    let resume_cmd = adapter.resume_command(&process, &meta);

                                    let id = format!("claude:{}", session_id);
                                    let project_name = std::path::Path::new(cwd)
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("unknown")
                                        .to_string();

                                    let now = chrono::Utc::now().timestamp();
                                    let tracked = TrackedSession {
                                        id: id.clone(),
                                        tool: "claude".to_string(),
                                        tool_color: "#8B5CF6".to_string(),
                                        pid,
                                        cwd: cwd.to_string(),
                                        project_name,
                                        started_at: now,
                                        last_seen: now,
                                        status: SessionStatus::Active,
                                        metadata: meta,
                                        resume_cmd,
                                        start_time: process.start_time,
                                        from_index: false,
                                    };

                                    let mut s = self.state.lock().await;
                                    s.sessions.insert(id, tracked);
                                    drop(s);
                                    let _ = self.app_handle.emit("sessions-updated", ());
                                }
                            }
                        }
                    }
                }
            }

            FsEvent::SessionFileDeleted { tool, path: _ } => {
                if tool == "claude" {
                    let _ = self.app_handle.emit("sessions-updated", ());
                }
            }
        }
    }

    /// Full process scan (Layer 3 - fallback)
    async fn full_scan(&self) {
        let processes = libproc_scanner::scan_processes();
        let mut s = self.state.lock().await;
        let now = chrono::Utc::now().timestamp();

        let mut alive_ids: HashSet<String> = HashSet::new();

        for process in &processes {
            let adapter = adapters::get_adapter(&process.tool);

            let compat = crate::libproc_scanner::DetectedProcess {
                pid: process.pid,
                tool: process.tool.clone(),
                tool_color: process.tool_color.clone(),
                comm: process.comm.clone(),
                cwd: process.cwd.clone(),
                args: process.args.clone(),
                start_time: process.start_time,
            };

            let meta = adapter.resolve_metadata(&compat);
            let resume_cmd = adapter.resume_command(&compat, &meta);

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
                existing.last_seen = now;
                if existing.status == SessionStatus::Active {
                    existing.metadata = meta;
                    existing.resume_cmd = resume_cmd;
                }
            } else {
                #[cfg(target_os = "macos")]
                self.kqueue.watch_pid(process.pid);

                let tracked = TrackedSession {
                    id: id.clone(),
                    tool: process.tool.clone(),
                    tool_color: process.tool_color.clone(),
                    pid: process.pid,
                    cwd: process.cwd.clone().unwrap_or_else(|| "~".to_string()),
                    project_name,
                    started_at: process.start_time as i64,
                    last_seen: now,
                    status: SessionStatus::Active,
                    metadata: meta,
                    resume_cmd,
                    start_time: process.start_time,
                    from_index: false,
                };
                s.sessions.insert(id, tracked);
            }
        }

        // Check for sessions that are Active but no longer running
        let mut newly_dead = Vec::new();
        for (id, session) in s.sessions.iter_mut() {
            if session.status == SessionStatus::Active && !alive_ids.contains(id) {
                if !libproc_scanner::is_pid_alive(session.pid) {
                    session.status = SessionStatus::Ended;
                    session.last_seen = now;
                    newly_dead.push(id.clone());
                }
            }
        }

        if !newly_dead.is_empty() {
            Self::detect_bulk_crash(&mut s, &newly_dead, now);
        }

        // GC old runtime-detected ended sessions (>24 hours)
        let cutoff = now - 86400;
        s.sessions.retain(|_, sess| {
            sess.from_index
                || matches!(sess.status, SessionStatus::Active | SessionStatus::Crashed)
                || sess.last_seen > cutoff
        });

        let has_crashes = s.sessions.values().any(|sess| sess.status == SessionStatus::Crashed);
        s.last_updated = now;
        drop(s);

        update_tray_icon(&self.app_handle, has_crashes);
        let _ = self.app_handle.emit("sessions-updated", ());
    }
}

fn send_crash_notification(_app: &tauri::AppHandle) {
    let title = "uhoh! Sessions crashed";
    let body = "Click the menu bar icon to recover.";

    #[cfg(target_os = "macos")]
    {
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

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("notify-send")
            .args(["--app-name=uhoh", title, body])
            .spawn();
    }

    // Windows: tray icon change serves as the primary indicator
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
