use crate::state::AppState;
use std::fs;
use std::path::PathBuf;

fn store_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".uhoh")
}

pub fn ensure_store_dir() {
    let dir = store_dir();
    let _ = fs::create_dir_all(&dir);
}

pub fn save_state(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    ensure_store_dir();
    let path = store_dir().join("state.json");
    let json = serde_json::to_string_pretty(state)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_state() -> Option<AppState> {
    let path = store_dir().join("state.json");
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}
