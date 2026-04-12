use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum FsEvent {
    SessionFileCreated { tool: String, path: PathBuf },
    SessionFileDeleted { tool: String, path: PathBuf },
}

/// Session directories to watch per tool
fn watch_targets() -> Vec<(&'static str, PathBuf)> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };

    let mut targets = Vec::new();

    // Claude: ~/.claude/sessions/
    let claude_dir = home.join(".claude").join("sessions");
    if claude_dir.exists() {
        targets.push(("claude", claude_dir));
    }

    // Codex: ~/.codex/sessions/
    let codex_dir = home.join(".codex").join("sessions");
    if codex_dir.exists() {
        targets.push(("codex", codex_dir));
    }

    // Codex alt: ~/.codex/ (for session_index.jsonl)
    let codex_root = home.join(".codex");
    if codex_root.exists() {
        targets.push(("codex", codex_root));
    }

    // Gemini: ~/.gemini/history/
    let gemini_dir = home.join(".gemini").join("history");
    if gemini_dir.exists() {
        targets.push(("gemini", gemini_dir));
    }

    // Kimi: ~/.kimi/sessions/
    let kimi_dir = home.join(".kimi").join("sessions");
    if kimi_dir.exists() {
        targets.push(("kimi", kimi_dir));
    }

    // Goose: ~/.local/share/goose/sessions/
    if let Some(data_dir) = dirs::data_local_dir() {
        let goose_dir = data_dir.join("goose").join("sessions");
        if goose_dir.exists() {
            targets.push(("goose", goose_dir));
        }
    }

    // OpenCode: ~/.local/share/opencode/storage/session/
    if let Some(data_dir) = dirs::data_local_dir() {
        let opencode_dir = data_dir.join("opencode").join("storage").join("session");
        if opencode_dir.exists() {
            targets.push(("opencode", opencode_dir));
        }
    }

    targets
}

/// Determine tool from file path by matching against known directories
fn tool_for_path(path: &std::path::Path) -> Option<String> {
    let path_str = path.to_string_lossy();
    if path_str.contains(".claude") {
        Some("claude".to_string())
    } else if path_str.contains(".codex") {
        Some("codex".to_string())
    } else if path_str.contains(".gemini") {
        Some("gemini".to_string())
    } else if path_str.contains(".kimi") {
        Some("kimi".to_string())
    } else if path_str.contains("goose") {
        Some("goose".to_string())
    } else if path_str.contains("opencode") {
        Some("opencode".to_string())
    } else {
        None
    }
}

/// Start watching all known session directories.
/// Returns an event receiver and the watcher handle (must be kept alive).
pub fn start_watching() -> (mpsc::UnboundedReceiver<FsEvent>, Option<RecommendedWatcher>) {
    let (tx, rx) = mpsc::unbounded_channel();

    let event_tx = tx.clone();
    let watcher_result = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                for path in &event.paths {
                    let tool = match tool_for_path(path) {
                        Some(t) => t,
                        None => continue,
                    };

                    match event.kind {
                        EventKind::Create(_) => {
                            let _ = event_tx.send(FsEvent::SessionFileCreated {
                                tool,
                                path: path.clone(),
                            });
                        }
                        EventKind::Remove(_) => {
                            let _ = event_tx.send(FsEvent::SessionFileDeleted {
                                tool,
                                path: path.clone(),
                            });
                        }
                        _ => {}
                    }
                }
            }
        },
        Config::default(),
    );

    let watcher = match watcher_result {
        Ok(mut w) => {
            let targets = watch_targets();
            for (tool, path) in &targets {
                if let Err(e) = w.watch(path, RecursiveMode::NonRecursive) {
                    eprintln!("uhoh: failed to watch {} dir {:?}: {}", tool, path, e);
                }
            }
            Some(w)
        }
        Err(e) => {
            eprintln!("uhoh: failed to create file watcher: {}", e);
            None
        }
    };

    (rx, watcher)
}
