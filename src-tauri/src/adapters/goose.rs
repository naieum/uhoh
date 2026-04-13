use super::ToolAdapter;
use crate::libproc_scanner::DetectedProcess;
use crate::state::SessionMeta;

pub struct GooseAdapter;

impl ToolAdapter for GooseAdapter {
    fn resolve_metadata(&self, _process: &DetectedProcess) -> SessionMeta {
        // Goose stores sessions in SQLite at ~/.local/share/goose/sessions/sessions.db
        // We use the CLI resume command which handles session lookup internally
        SessionMeta::default()
    }

    fn resume_command(&self, process: &DetectedProcess, meta: &SessionMeta) -> String {
        let cwd = process.cwd.as_deref().unwrap_or("~");
        if let Some(ref name) = meta.session_name {
            format!("cd {} && goose session -r --name {}", cwd, name)
        } else {
            format!("cd {} && goose session -r", cwd)
        }
    }
}
