use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;

pub struct KimiAdapter;

impl ToolAdapter for KimiAdapter {
    fn tool_name(&self) -> &'static str {
        "kimi"
    }

    fn resolve_metadata(&self, _process: &DetectedProcess) -> SessionMeta {
        // Kimi stores sessions in ~/.kimi/sessions/ with wire.jsonl and context.jsonl
        // Could read from there for richer metadata in the future
        SessionMeta::default()
    }

    fn resume_command(&self, process: &DetectedProcess, _meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        // Kimi uses a web UI for session management
        format!("cd {} && kimi", cwd)
    }
}
