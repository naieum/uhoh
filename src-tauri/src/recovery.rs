use crate::state::TrackedSession;
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
pub struct TerminalTool {
    pub id: String,
    pub name: String,
    pub available: bool,
}

struct TerminalDef {
    id: &'static str,
    name: &'static str,
    check: CheckMethod,
}

enum CheckMethod {
    #[cfg(target_os = "macos")]
    App(&'static str),
    Bin(&'static str),
    Always,
}

// ============================================================
// Platform-specific terminal lists
// ============================================================

#[cfg(target_os = "macos")]
const TERMINALS: &[TerminalDef] = &[
    TerminalDef { id: "terminal", name: "Terminal.app", check: CheckMethod::Always },
    TerminalDef { id: "tmux",     name: "tmux (grid)",  check: CheckMethod::Bin("tmux") },
    TerminalDef { id: "iterm",    name: "iTerm2",       check: CheckMethod::App("iTerm") },
    TerminalDef { id: "warp",     name: "Warp",         check: CheckMethod::App("Warp") },
    TerminalDef { id: "ghostty",  name: "Ghostty",      check: CheckMethod::App("Ghostty") },
    TerminalDef { id: "alacritty",name: "Alacritty",    check: CheckMethod::App("Alacritty") },
    TerminalDef { id: "kitty",    name: "Kitty",        check: CheckMethod::App("kitty") },
    TerminalDef { id: "wezterm",  name: "WezTerm",      check: CheckMethod::App("WezTerm") },
    TerminalDef { id: "hyper",    name: "Hyper",        check: CheckMethod::App("Hyper") },
    TerminalDef { id: "tabby",    name: "Tabby",        check: CheckMethod::App("Tabby") },
    TerminalDef { id: "rio",      name: "Rio",          check: CheckMethod::App("Rio") },
    TerminalDef { id: "wave",     name: "Wave",         check: CheckMethod::App("Wave") },
];

#[cfg(target_os = "linux")]
const TERMINALS: &[TerminalDef] = &[
    TerminalDef { id: "tmux",            name: "tmux (grid)",     check: CheckMethod::Bin("tmux") },
    TerminalDef { id: "gnome-terminal",  name: "GNOME Terminal",  check: CheckMethod::Bin("gnome-terminal") },
    TerminalDef { id: "konsole",         name: "Konsole",         check: CheckMethod::Bin("konsole") },
    TerminalDef { id: "kitty",           name: "Kitty",           check: CheckMethod::Bin("kitty") },
    TerminalDef { id: "alacritty",       name: "Alacritty",       check: CheckMethod::Bin("alacritty") },
    TerminalDef { id: "wezterm",         name: "WezTerm",         check: CheckMethod::Bin("wezterm") },
    TerminalDef { id: "foot",            name: "Foot",            check: CheckMethod::Bin("foot") },
    TerminalDef { id: "xfce4-terminal",  name: "Xfce Terminal",   check: CheckMethod::Bin("xfce4-terminal") },
    TerminalDef { id: "xterm",           name: "xterm",           check: CheckMethod::Bin("xterm") },
];

#[cfg(target_os = "windows")]
const TERMINALS: &[TerminalDef] = &[
    TerminalDef { id: "wt",   name: "Windows Terminal", check: CheckMethod::Bin("wt") },
    TerminalDef { id: "pwsh", name: "PowerShell",       check: CheckMethod::Bin("pwsh") },
    TerminalDef { id: "cmd",  name: "Command Prompt",   check: CheckMethod::Always },
];

// ============================================================
// Terminal detection
// ============================================================

fn is_bin_available(name: &str) -> bool {
    #[cfg(unix)]
    {
        Command::new("which").arg(name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        Command::new("where").arg(name)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

pub fn detect_tools() -> Vec<TerminalTool> {
    TERMINALS
        .iter()
        .map(|t| {
            let available = match &t.check {
                CheckMethod::Always => true,
                #[cfg(target_os = "macos")]
                CheckMethod::App(name) => {
                    std::path::Path::new(&format!("/Applications/{}.app", name)).exists()
                }
                CheckMethod::Bin(name) => is_bin_available(name),
            };
            TerminalTool { id: t.id.to_string(), name: t.name.to_string(), available }
        })
        .filter(|t| t.available)
        .collect()
}

// ============================================================
// Shared restore logic
// ============================================================

pub fn restore_one(session: &TrackedSession, tool: &str) -> Result<(), Box<dyn std::error::Error>> {
    let cmd = &session.resume_cmd;
    match tool {
        "tmux" => open_in_tmux_window(cmd, &session.project_name),
        _ => open_via_script(cmd, tool),
    }
}

pub fn restore_batch(cmds: &[String], tool: &str) -> Result<(), Box<dyn std::error::Error>> {
    if cmds.is_empty() { return Ok(()); }
    if cmds.len() == 1 { return open_via_script(&cmds[0], tool); }

    match tool {
        "tmux" => {
            for cmd in cmds {
                open_via_script(cmd, tool)?;
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Ok(())
        }
        #[cfg(target_os = "macos")]
        "terminal" => open_tabs_terminal(cmds),
        #[cfg(target_os = "macos")]
        "iterm" => open_tabs_iterm(cmds),
        _ => {
            for cmd in cmds {
                open_via_script(cmd, tool)?;
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            Ok(())
        }
    }
}

pub fn restore_grid(sessions: &[&TrackedSession]) -> Result<(), Box<dyn std::error::Error>> {
    if sessions.is_empty() { return Ok(()); }

    let session_name = "uhoh-restore";
    let _ = Command::new("tmux").args(["kill-session", "-t", session_name]).output();

    Command::new("tmux")
        .args(["new-session", "-d", "-s", session_name, "-n", "grid"])
        .output()?;
    Command::new("tmux")
        .args(["send-keys", "-t", &format!("{}:0.0", session_name), &sessions[0].resume_cmd, "Enter"])
        .output()?;

    for (i, session) in sessions.iter().enumerate().skip(1) {
        Command::new("tmux")
            .args(["split-window", "-t", &format!("{}:0.0", session_name)])
            .output()?;
        Command::new("tmux")
            .args(["select-layout", "-t", &format!("{}:0", session_name), "tiled"])
            .output()?;
        Command::new("tmux")
            .args(["send-keys", "-t", &format!("{}:0.{}", session_name, i), &session.resume_cmd, "Enter"])
            .output()?;
    }

    let _ = Command::new("tmux")
        .args(["select-layout", "-t", &format!("{}:0", session_name), "tiled"])
        .output();
    let _ = Command::new("tmux")
        .args(["select-pane", "-t", &format!("{}:0.0", session_name)])
        .output();

    open_new_terminal_window(&format!("tmux attach -t {}", session_name))
}

// ============================================================
// macOS: open commands in terminals
// ============================================================

#[cfg(target_os = "macos")]
fn open_via_script(cmd: &str, tool: &str) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = std::env::temp_dir().join("uhoh-restore");
    std::fs::create_dir_all(&tmp_dir)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let script_path = tmp_dir.join(format!("restore-{}.sh", ts));
    std::fs::write(&script_path, format!("#!/bin/bash\nrm -f \"$0\"\n{}\n", cmd))?;
    Command::new("chmod").args(["+x", script_path.to_str().unwrap()]).output()?;

    let escaped_path = script_path.to_str().unwrap().replace('"', "\\\"");

    match tool {
        "terminal" | "" => {
            let script = format!(
                r#"tell application "Terminal"
    activate
    if (count of windows) > 0 then
        do script "{}" in front window
    else
        do script "{}"
    end if
end tell"#,
                escaped_path, escaped_path
            );
            let output = Command::new("osascript").args(["-e", &script]).output()?;
            if !output.status.success() {
                let cmd_path = tmp_dir.join(format!("restore-{}.command", ts));
                std::fs::rename(&script_path, &cmd_path)?;
                Command::new("open").arg(&cmd_path).output()?;
            }
        }
        other => {
            let app_name = match other {
                "iterm" => "iTerm", "warp" => "Warp", "ghostty" => "Ghostty",
                "alacritty" => "Alacritty", "kitty" => "kitty", "wezterm" => "WezTerm",
                "hyper" => "Hyper", "tabby" => "Tabby", "rio" => "Rio", "wave" => "Wave",
                _ => "Terminal",
            };
            let cmd_path = tmp_dir.join(format!("restore-{}.command", ts));
            std::fs::rename(&script_path, &cmd_path)?;
            Command::new("open")
                .args(["-a", app_name, cmd_path.to_str().unwrap()])
                .output()?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_tabs_terminal(cmds: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = std::env::temp_dir().join("uhoh-restore");
    std::fs::create_dir_all(&tmp_dir)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let mut script_paths = Vec::new();
    for (i, cmd) in cmds.iter().enumerate() {
        let path = tmp_dir.join(format!("restore-{}-{}.sh", ts, i));
        std::fs::write(&path, format!("#!/bin/bash\nrm -f \"$0\"\n{}\n", cmd))?;
        Command::new("chmod").args(["+x", path.to_str().unwrap()]).output()?;
        script_paths.push(path);
    }

    let mut applescript = String::from("tell application \"Terminal\"\n    activate\n");
    applescript.push_str(&format!(
        "    set newTab to do script \"{}\"\n    set newWindow to window 1\n",
        script_paths[0].to_str().unwrap().replace('"', "\\\"")
    ));
    for path in &script_paths[1..] {
        applescript.push_str(&format!(
            "    do script \"{}\" in newWindow\n",
            path.to_str().unwrap().replace('"', "\\\"")
        ));
    }
    applescript.push_str("end tell\n");

    let output = Command::new("osascript").args(["-e", &applescript]).output()?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("AppleScript failed: {}", err).into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn open_tabs_iterm(cmds: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let tmp_dir = std::env::temp_dir().join("uhoh-restore");
    std::fs::create_dir_all(&tmp_dir)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let mut script_paths = Vec::new();
    for (i, cmd) in cmds.iter().enumerate() {
        let path = tmp_dir.join(format!("restore-{}-{}.sh", ts, i));
        std::fs::write(&path, format!("#!/bin/bash\nrm -f \"$0\"\n{}\n", cmd))?;
        Command::new("chmod").args(["+x", path.to_str().unwrap()]).output()?;
        script_paths.push(path);
    }

    let mut applescript = String::from("tell application \"iTerm2\"\n    activate\n    tell current window\n");
    for (i, path) in script_paths.iter().enumerate() {
        if i == 0 {
            applescript.push_str(&format!(
                "        tell current session\n            write text \"{}\"\n        end tell\n",
                path.to_str().unwrap().replace('"', "\\\"")
            ));
        } else {
            applescript.push_str(&format!(
                "        create tab with default profile\n\
                         tell current session\n            write text \"{}\"\n        end tell\n",
                path.to_str().unwrap().replace('"', "\\\"")
            ));
        }
    }
    applescript.push_str("    end tell\nend tell\n");

    Command::new("osascript").args(["-e", &applescript]).output()?;
    Ok(())
}

// ============================================================
// Linux: open commands in terminals
// ============================================================

#[cfg(target_os = "linux")]
fn open_via_script(cmd: &str, tool: &str) -> Result<(), Box<dyn std::error::Error>> {
    match tool {
        "gnome-terminal" => {
            Command::new("gnome-terminal").args(["--", "bash", "-c", cmd]).spawn()?;
        }
        "konsole" => {
            Command::new("konsole").args(["-e", "bash", "-c", cmd]).spawn()?;
        }
        "xfce4-terminal" => {
            Command::new("xfce4-terminal").args(["-e", &format!("bash -c '{}'", cmd.replace('\'', "'\\''"))]).spawn()?;
        }
        "kitty" => {
            Command::new("kitty").args(["bash", "-c", cmd]).spawn()?;
        }
        "alacritty" => {
            Command::new("alacritty").args(["-e", "bash", "-c", cmd]).spawn()?;
        }
        "wezterm" => {
            Command::new("wezterm").args(["start", "--", "bash", "-c", cmd]).spawn()?;
        }
        "foot" => {
            Command::new("foot").args(["bash", "-c", cmd]).spawn()?;
        }
        "xterm" | _ => {
            Command::new("xterm").args(["-e", &format!("bash -c '{}'", cmd.replace('\'', "'\\''"))]).spawn()?;
        }
    }
    Ok(())
}

// ============================================================
// Windows: open commands in terminals
// ============================================================

#[cfg(target_os = "windows")]
fn open_via_script(cmd: &str, tool: &str) -> Result<(), Box<dyn std::error::Error>> {
    match tool {
        "wt" => {
            Command::new("wt").args(["new-tab", "cmd", "/K", cmd]).spawn()?;
        }
        "pwsh" => {
            Command::new("pwsh").args(["-NoExit", "-Command", cmd]).spawn()?;
        }
        "cmd" | _ => {
            Command::new("cmd").args(["/C", "start", "cmd", "/K", cmd]).spawn()?;
        }
    }
    Ok(())
}

// ============================================================
// tmux window management (cross-platform, Unix + Windows via WSL)
// ============================================================

fn open_in_tmux_window(cmd: &str, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let session_name = "uhoh-restore";
    let exists = Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if exists {
        Command::new("tmux")
            .args(["new-window", "-t", session_name, "-n", name])
            .output()?;
        Command::new("tmux")
            .args(["send-keys", "-t", &format!("{}:{}", session_name, name), cmd, "Enter"])
            .output()?;
    } else {
        Command::new("tmux")
            .args(["new-session", "-d", "-s", session_name, "-n", name])
            .output()?;
        Command::new("tmux")
            .args(["send-keys", "-t", &format!("{}:0", session_name), cmd, "Enter"])
            .output()?;
        open_new_terminal_window(&format!("tmux attach -t {}", session_name))?;
    }
    Ok(())
}

/// Open a new terminal window with the given command
fn open_new_terminal_window(cmd: &str) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        let tmp_dir = std::env::temp_dir().join("uhoh-restore");
        std::fs::create_dir_all(&tmp_dir)?;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let script_path = tmp_dir.join(format!("restore-{}.command", ts));
        std::fs::write(&script_path, format!("#!/bin/bash\nrm -f \"$0\"\n{}\n", cmd))?;
        Command::new("chmod").args(["+x", script_path.to_str().unwrap()]).output()?;
        Command::new("open").arg(&script_path).output()?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try common terminals in order of preference
        let wrapped = format!("bash -c '{}'", cmd.replace('\'', "'\\''"));
        if is_bin_available("gnome-terminal") {
            Command::new("gnome-terminal").args(["--", "bash", "-c", cmd]).spawn()?;
        } else if is_bin_available("konsole") {
            Command::new("konsole").args(["-e", "bash", "-c", cmd]).spawn()?;
        } else if is_bin_available("kitty") {
            Command::new("kitty").args(["bash", "-c", cmd]).spawn()?;
        } else if is_bin_available("xfce4-terminal") {
            Command::new("xfce4-terminal").args(["-e", &wrapped]).spawn()?;
        } else {
            Command::new("xterm").args(["-e", &wrapped]).spawn()?;
        }
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", "cmd", "/K", cmd]).spawn()?;
    }

    Ok(())
}
