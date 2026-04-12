use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;

pub struct GeminiAdapter;

impl ToolAdapter for GeminiAdapter {
    fn tool_name(&self) -> &'static str {
        "gemini"
    }

    fn resolve_metadata(&self, process: &DetectedProcess) -> SessionMeta {
        // Gemini stores sessions in ~/.gemini/history/ and project-specific chats
        // For now, extract what we can from the process args
        let summary = if process.args.contains("--resume") || process.args.contains("-r") {
            Some("Resumed session".to_string())
        } else {
            None
        };

        SessionMeta {
            summary,
            ..Default::default()
        }
    }

    fn resume_command(&self, process: &DetectedProcess, _meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        // gemini -r resumes the most recent session in the current directory
        format!("cd {} && gemini -r", cwd)
    }
}
