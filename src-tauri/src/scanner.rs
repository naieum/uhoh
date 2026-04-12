use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct DetectedProcess {
    pub pid: u32,
    pub tool: String,
    pub tool_color: String,
    pub comm: String,
    pub cwd: Option<String>,
    pub args: String,
}

/// Known AI coding CLI tool names mapped to (display_name, color)
fn known_tools() -> HashMap<&'static str, (&'static str, &'static str)> {
    let mut m = HashMap::new();
    m.insert("claude", ("claude", "#8B5CF6"));     // purple
    m.insert("gemini", ("gemini", "#4285F4"));     // blue
    m.insert("codex", ("codex", "#10A37F"));       // green
    m.insert("opencode", ("opencode", "#06B6D4")); // cyan
    m.insert("kimi", ("kimi", "#EF4444"));         // red
    m.insert("goose", ("goose", "#F59E0B"));       // amber
    m.insert("aider", ("aider", "#EC4899"));       // pink
    m
}

/// Scan all running processes for known AI coding tools
pub fn scan_processes() -> Vec<DetectedProcess> {
    let tools = known_tools();
    let mut results = Vec::new();

    // Get process list with PID, PPID, command name, and full args
    let output = match Command::new("ps")
        .args(["-eo", "pid,ppid,comm,args"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return results,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }

        let pid: u32 = match parts[0].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let comm = parts[2];
        // Extract just the binary name from the full path
        let bin_name = comm.rsplit('/').next().unwrap_or(comm);

        if let Some(&(tool_name, color)) = tools.get(bin_name) {
            let args = parts[3..].join(" ");

            // Skip if this is uhoh itself or a subprocess of uhoh
            if args.contains("uhoh") {
                continue;
            }

            let cwd = get_cwd_for_pid(pid);

            results.push(DetectedProcess {
                pid,
                tool: tool_name.to_string(),
                tool_color: color.to_string(),
                comm: comm.to_string(),
                cwd,
                args,
            });
        }
    }

    results
}

/// Get the current working directory of a process on macOS
fn get_cwd_for_pid(pid: u32) -> Option<String> {
    // Use lsof to get the CWD on macOS
    let output = Command::new("lsof")
        .args(["-p", &pid.to_string(), "-Fn", "-d", "cwd"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix('n') {
            if path.starts_with('/') {
                return Some(path.to_string());
            }
        }
    }
    None
}

/// Check if a PID is still alive
pub fn is_pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

// Need libc for kill()
extern crate libc;
