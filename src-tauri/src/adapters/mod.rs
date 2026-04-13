pub mod aider;
pub mod claude;
pub mod codex;
pub mod gemini;
pub mod goose;
pub mod kimi;
pub mod opencode;

use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;

/// Trait for tool-specific session handling
pub trait ToolAdapter: Send + Sync {
    fn resolve_metadata(&self, process: &DetectedProcess) -> SessionMeta;
    fn resume_command(&self, process: &DetectedProcess, meta: &SessionMeta) -> String;
}

/// Get the appropriate adapter for a tool name
pub fn get_adapter(tool: &str) -> Box<dyn ToolAdapter> {
    match tool {
        "claude" => Box::new(claude::ClaudeAdapter),
        "gemini" => Box::new(gemini::GeminiAdapter),
        "codex" => Box::new(codex::CodexAdapter),
        "opencode" => Box::new(opencode::OpenCodeAdapter),
        "kimi" => Box::new(kimi::KimiAdapter),
        "goose" => Box::new(goose::GooseAdapter),
        "aider" => Box::new(aider::AiderAdapter),
        _ => Box::new(GenericAdapter),
    }
}

/// Fallback adapter for unknown tools
struct GenericAdapter;

impl ToolAdapter for GenericAdapter {
    fn resolve_metadata(&self, _process: &DetectedProcess) -> SessionMeta {
        SessionMeta::default()
    }

    fn resume_command(&self, process: &DetectedProcess, _meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        format!("cd {} && {}", cwd, process.comm)
    }
}
