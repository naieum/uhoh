use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;

pub struct AiderAdapter;

impl ToolAdapter for AiderAdapter {
    fn tool_name(&self) -> &'static str {
        "aider"
    }

    fn resolve_metadata(&self, _process: &DetectedProcess) -> SessionMeta {
        // Aider has limited session persistence (.aider.chat.history.md)
        // No native resume support
        SessionMeta {
            summary: Some("No resume support - history in .aider.chat.history.md".to_string()),
            ..Default::default()
        }
    }

    fn resume_command(&self, process: &DetectedProcess, _meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        // Aider doesn't support resume, just restart in the same directory
        format!("cd {} && aider", cwd)
    }
}
