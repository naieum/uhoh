use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;

pub struct OpenCodeAdapter;

impl ToolAdapter for OpenCodeAdapter {
    fn tool_name(&self) -> &'static str {
        "opencode"
    }

    fn resolve_metadata(&self, _process: &DetectedProcess) -> SessionMeta {
        // OpenCode stores sessions in ~/.local/share/opencode/storage/session/
        // Could read from there for richer metadata in the future
        SessionMeta::default()
    }

    fn resume_command(&self, process: &DetectedProcess, meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        if let Some(ref sid) = meta.session_id {
            format!("cd {} && opencode -s {}", cwd, sid)
        } else {
            format!("cd {} && opencode", cwd)
        }
    }
}
