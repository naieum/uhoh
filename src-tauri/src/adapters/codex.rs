use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

pub struct CodexAdapter;

fn codex_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".codex"))
}

/// Try to find the most recent session from the Codex session index
fn find_recent_session(cwd: &str) -> Option<(String, Option<String>)> {
    let dir = codex_dir()?;
    let index_path = dir.join("session_index.jsonl");
    let file = fs::File::open(index_path).ok()?;
    let reader = BufReader::new(file);

    let mut best: Option<(String, Option<String>, String)> = None;

    for line in reader.lines().map_while(Result::ok) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            let id = val.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            let name = val.get("thread_name").and_then(|v| v.as_str());
            let updated = val
                .get("updated_at")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            // Try to match by checking if the session was used in this directory
            // Codex doesn't always store cwd in the index, so we take the most recent
            if best
                .as_ref()
                .map(|(_, _, ts)| updated > ts.as_str())
                .unwrap_or(true)
            {
                best = Some((
                    id.to_string(),
                    name.map(|s| s.to_string()),
                    updated.to_string(),
                ));
            }
        }
    }

    let _ = cwd; // May use for filtering in the future
    best.map(|(id, name, _)| (id, name))
}

impl ToolAdapter for CodexAdapter {
    fn tool_name(&self) -> &'static str {
        "codex"
    }

    fn resolve_metadata(&self, process: &DetectedProcess) -> SessionMeta {
        let cwd = process.cwd.as_deref().unwrap_or("~");

        if let Some((session_id, session_name)) = find_recent_session(cwd) {
            return SessionMeta {
                session_id: Some(session_id),
                session_name,
                summary: None,
                ..Default::default()
            };
        }

        SessionMeta::default()
    }

    fn resume_command(&self, process: &DetectedProcess, meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        if let Some(ref sid) = meta.session_id {
            format!("cd {} && codex resume {}", cwd, sid)
        } else {
            format!("cd {} && codex resume --last", cwd)
        }
    }
}
