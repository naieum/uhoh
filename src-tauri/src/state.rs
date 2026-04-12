use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    Active,
    Crashed,
    Ended,
    Recovered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub summary: Option<String>,
    pub first_prompt: Option<String>,
    pub message_count: Option<u32>,
    pub git_branch: Option<String>,
    pub session_id: Option<String>,
    pub session_name: Option<String>,
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self {
            summary: None,
            first_prompt: None,
            message_count: None,
            git_branch: None,
            session_id: None,
            session_name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedSession {
    pub id: String,
    pub tool: String,
    pub tool_color: String,
    pub pid: u32,
    pub cwd: String,
    pub project_name: String,
    pub started_at: i64,
    pub last_seen: i64,
    pub status: SessionStatus,
    pub metadata: SessionMeta,
    pub resume_cmd: String,
    pub start_time: u64, // Process start time for PID reuse detection
    #[serde(default)]
    pub from_index: bool, // Loaded from tool's session index - don't GC
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashEvent {
    pub id: String,
    pub detected_at: i64,
    pub sessions: Vec<String>, // session IDs
    pub dismissed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub sessions: HashMap<String, TrackedSession>,
    pub crashes: Vec<CrashEvent>,
    pub last_updated: i64,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            crashes: Vec::new(),
            last_updated: chrono::Utc::now().timestamp(),
        }
    }
}
