use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

pub struct ClaudeAdapter;

#[derive(Deserialize)]
struct ClaudeSessionFile {
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "startedAt")]
    _started_at: Option<u64>,
}

#[derive(Deserialize)]
struct SessionsIndex {
    entries: Option<Vec<SessionIndexEntry>>,
}

#[derive(Deserialize)]
struct SessionIndexEntry {
    #[serde(rename = "sessionId")]
    session_id: String,
    summary: Option<String>,
    #[serde(rename = "firstPrompt")]
    first_prompt: Option<String>,
    #[serde(rename = "messageCount")]
    message_count: Option<u32>,
    #[serde(rename = "gitBranch")]
    git_branch: Option<String>,
}

fn claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

/// Read the PID-based session file for a Claude process
fn read_session_file(pid: u32) -> Option<ClaudeSessionFile> {
    let dir = claude_dir()?;
    let path = dir.join("sessions").join(format!("{}.json", pid));
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Encode a path the way Claude does: /foo/bar -> -foo-bar
fn encode_project_path(path: &str) -> String {
    path.replace('/', "-")
}

/// Read sessions-index.json for a project and find a specific session
fn read_session_metadata(cwd: &str, session_id: &str) -> Option<SessionIndexEntry> {
    let dir = claude_dir()?;
    let encoded = encode_project_path(cwd);
    let index_path = dir.join("projects").join(&encoded).join("sessions-index.json");
    let content = fs::read_to_string(index_path).ok()?;
    let index: SessionsIndex = serde_json::from_str(&content).ok()?;

    index
        .entries?
        .into_iter()
        .find(|e| e.session_id == session_id)
}

impl ToolAdapter for ClaudeAdapter {
    fn resolve_metadata(&self, process: &DetectedProcess) -> SessionMeta {
        // Try to read the PID session file first
        let session_file = read_session_file(process.pid);
        let session_id = session_file.as_ref().and_then(|s| s.session_id.clone());

        let cwd = process
            .cwd
            .clone()
            .or_else(|| session_file.as_ref().and_then(|s| s.cwd.clone()));

        // If we have both CWD and session_id, look up rich metadata
        if let (Some(ref cwd_path), Some(ref sid)) = (&cwd, &session_id) {
            if let Some(entry) = read_session_metadata(cwd_path, sid) {
                return SessionMeta {
                    summary: entry.summary,
                    first_prompt: entry.first_prompt,
                    message_count: entry.message_count,
                    git_branch: entry.git_branch,
                    session_id: Some(sid.clone()),
                    session_name: None,
                };
            }
        }

        SessionMeta {
            session_id,
            ..Default::default()
        }
    }

    fn resume_command(&self, process: &DetectedProcess, meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        if let Some(ref sid) = meta.session_id {
            format!("cd {} && claude --resume {}", cwd, sid)
        } else {
            format!("cd {} && claude --continue", cwd)
        }
    }

}
