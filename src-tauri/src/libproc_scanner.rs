#[derive(Debug, Clone)]
pub struct DetectedProcess {
    pub pid: u32,
    pub tool: String,
    pub tool_color: String,
    pub comm: String,
    pub cwd: Option<String>,
    pub args: String,
    pub start_time: u64,
}

struct ToolMatch {
    name: &'static str,
    color: &'static str,
    /// Substrings to match in the executable path
    path_patterns: &'static [&'static str],
}

const KNOWN_TOOLS: &[ToolMatch] = &[
    ToolMatch { name: "claude",    color: "#8B5CF6", path_patterns: &["/claude/versions/", "/bin/claude"] },
    ToolMatch { name: "gemini",    color: "#4285F4", path_patterns: &["/gemini/", "/bin/gemini"] },
    // Codex App must be checked BEFORE generic codex to match first
    ToolMatch { name: "codex app", color: "#10A37F", path_patterns: &["/Codex.app/"] },
    ToolMatch { name: "codex",     color: "#10A37F", path_patterns: &["/codex/", "/bin/codex"] },
    ToolMatch { name: "opencode",  color: "#06B6D4", path_patterns: &["/opencode/", "/bin/opencode"] },
    ToolMatch { name: "kimi",      color: "#EF4444", path_patterns: &["/kimi/", "/bin/kimi", "/kimi-cli/"] },
    ToolMatch { name: "goose",     color: "#F59E0B", path_patterns: &["/goose/", "/bin/goose"] },
    ToolMatch { name: "aider",     color: "#EC4899", path_patterns: &["/aider/", "/bin/aider"] },
];

/// Script interpreters that might be running AI tools
const SCRIPT_INTERPRETERS: &[&str] = &["node", "python", "python3", "deno", "bun"];

fn match_tool(path: &str) -> Option<(&'static str, &'static str)> {
    // Skip Electron helper/child processes
    if path.contains("Helper.app/") || path.contains("Helper (") ||
       path.contains("crashpad_handler") || path.contains("Sparkle") ||
       path.contains("Updater.app/") || path.ends_with("/Resources/codex") {
        return None;
    }

    for tool in KNOWN_TOOLS {
        for pattern in tool.path_patterns {
            if path.contains(pattern) {
                return Some((tool.name, tool.color));
            }
        }
    }
    None
}

/// Check if a binary name is a script interpreter (node, python, etc.)
fn is_interpreter(path: &str) -> bool {
    let bin_name = path.rsplit('/').next().unwrap_or(path);
    // On Windows, also handle backslash separators
    let bin_name = bin_name.rsplit('\\').next().unwrap_or(bin_name);
    SCRIPT_INTERPRETERS.iter().any(|&i| bin_name == i)
}

// ============================================================
// macOS: native libproc FFI for fast process scanning
// ============================================================
#[cfg(target_os = "macos")]
mod macos {
    use super::*;

    extern "C" {
        fn proc_listallpids(buffer: *mut libc::c_void, buffersize: libc::c_int) -> libc::c_int;
        fn proc_pidpath(pid: libc::c_int, buffer: *mut libc::c_void, buffersize: u32) -> libc::c_int;
        fn proc_pidinfo(
            pid: libc::c_int,
            flavor: libc::c_int,
            arg: u64,
            buffer: *mut libc::c_void,
            buffersize: libc::c_int,
        ) -> libc::c_int;
    }

    const PROC_PIDVNODEPATHINFO: libc::c_int = 9;
    const PROC_PIDTBSDINFO: libc::c_int = 3;

    #[repr(C)]
    struct VnodeInfoPath {
        _vip_vi: [u8; 152],
        vip_path: [u8; 1024],
    }

    #[repr(C)]
    struct ProcVnodePathInfo {
        pvi_cdir: VnodeInfoPath,
        _pvi_rdir: VnodeInfoPath,
    }

    #[repr(C)]
    struct ProcBsdInfo {
        _pbi_flags: u32,
        _pbi_status: u32,
        _pbi_xstatus: u32,
        _pbi_pid: u32,
        _pbi_ppid: u32,
        _pbi_uid: libc::uid_t,
        _pbi_gid: libc::gid_t,
        _pbi_ruid: libc::uid_t,
        _pbi_rgid: libc::gid_t,
        _pbi_svuid: libc::uid_t,
        _pbi_svgid: libc::gid_t,
        _rfu_1: u32,
        _pbi_comm: [u8; 16],
        _pbi_name: [u8; 32],
        _pbi_nfiles: u32,
        _pbi_pgid: u32,
        _pbi_pjobc: u32,
        _e_tdev: u32,
        _e_tpgid: u32,
        _pbi_nice: i32,
        pbi_start_tvsec: u64,
        _pbi_start_tvusec: u64,
    }

    /// Get command line arguments for a PID using sysctl KERN_PROCARGS2
    fn get_proc_args(pid: i32) -> Option<String> {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
        let mut size: libc::size_t = 0;

        let ret = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(), 3,
                std::ptr::null_mut(), &mut size,
                std::ptr::null_mut(), 0,
            )
        };
        if ret != 0 || size == 0 { return None; }

        let mut buf = vec![0u8; size];
        let ret = unsafe {
            libc::sysctl(
                mib.as_mut_ptr(), 3,
                buf.as_mut_ptr() as *mut libc::c_void, &mut size,
                std::ptr::null_mut(), 0,
            )
        };
        if ret != 0 { return None; }
        if size < 4 { return None; }

        let argc = u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
        let mut pos = 4;
        while pos < size && buf[pos] != 0 { pos += 1; }
        while pos < size && buf[pos] == 0 { pos += 1; }

        let mut args = Vec::new();
        for _ in 0..argc {
            let start = pos;
            while pos < size && buf[pos] != 0 { pos += 1; }
            if let Ok(arg) = std::str::from_utf8(&buf[start..pos]) {
                args.push(arg.to_string());
            }
            pos += 1;
        }
        Some(args.join(" "))
    }

    pub fn scan_processes() -> Vec<DetectedProcess> {
        let mut results = Vec::new();
        let mut seen_pids = std::collections::HashSet::new();

        let mut pids = vec![0i32; 8192];
        let count = unsafe {
            proc_listallpids(
                pids.as_mut_ptr() as *mut libc::c_void,
                (pids.len() * std::mem::size_of::<i32>()) as libc::c_int,
            )
        };
        if count <= 0 { return results; }

        let mut path_buf = [0u8; 4096];

        for &pid in &pids[..count as usize] {
            if pid <= 0 { continue; }

            let path_len = unsafe {
                proc_pidpath(pid, path_buf.as_mut_ptr() as *mut libc::c_void, path_buf.len() as u32)
            };
            if path_len <= 0 { continue; }

            let path = match std::str::from_utf8(&path_buf[..path_len as usize]) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if let Some((tool_name, color)) = match_tool(path) {
                if seen_pids.insert(pid as u32) {
                    let cwd = get_proc_cwd(pid);
                    let start_time = get_pid_start_time(pid as u32).unwrap_or(0);
                    results.push(DetectedProcess {
                        pid: pid as u32,
                        tool: tool_name.to_string(),
                        tool_color: color.to_string(),
                        comm: path.to_string(),
                        cwd,
                        args: String::new(),
                        start_time,
                    });
                }
                continue;
            }

            if is_interpreter(path) {
                if let Some(args) = get_proc_args(pid) {
                    if let Some((tool_name, color)) = match_tool(&args) {
                        if seen_pids.insert(pid as u32) {
                            let cwd = get_proc_cwd(pid);
                            let start_time = get_pid_start_time(pid as u32).unwrap_or(0);
                            results.push(DetectedProcess {
                                pid: pid as u32,
                                tool: tool_name.to_string(),
                                tool_color: color.to_string(),
                                comm: args.clone(),
                                cwd,
                                args,
                                start_time,
                            });
                        }
                    }
                }
            }
        }
        results
    }

    fn get_proc_cwd(pid: i32) -> Option<String> {
        let mut info: ProcVnodePathInfo = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            proc_pidinfo(
                pid, PROC_PIDVNODEPATHINFO, 0,
                &mut info as *mut _ as *mut libc::c_void,
                std::mem::size_of::<ProcVnodePathInfo>() as libc::c_int,
            )
        };
        if ret <= 0 { return None; }
        let path_bytes = &info.pvi_cdir.vip_path;
        let end = path_bytes.iter().position(|&b| b == 0).unwrap_or(1024);
        if end == 0 { return None; }
        Some(String::from_utf8_lossy(&path_bytes[..end]).into_owned())
    }

    pub fn get_pid_start_time(pid: u32) -> Option<u64> {
        let mut info: ProcBsdInfo = unsafe { std::mem::zeroed() };
        let ret = unsafe {
            proc_pidinfo(
                pid as libc::c_int, PROC_PIDTBSDINFO, 0,
                &mut info as *mut _ as *mut libc::c_void,
                std::mem::size_of::<ProcBsdInfo>() as libc::c_int,
            )
        };
        if ret <= 0 { return None; }
        Some(info.pbi_start_tvsec)
    }
}

// ============================================================
// Linux / Windows: sysinfo-based process scanning
// ============================================================
#[cfg(not(target_os = "macos"))]
mod generic {
    use super::*;
    use sysinfo::System;

    pub fn scan_processes() -> Vec<DetectedProcess> {
        let mut results = Vec::new();
        let mut seen_pids = std::collections::HashSet::new();
        let s = System::new_all();

        for (pid, process) in s.processes() {
            let pid_u32 = pid.as_u32();

            let exe_path = match process.exe() {
                Some(p) => p.to_string_lossy().to_string(),
                None => continue,
            };

            // Direct match on executable path
            if let Some((tool_name, color)) = match_tool(&exe_path) {
                if seen_pids.insert(pid_u32) {
                    let cwd = process.cwd().map(|p| p.to_string_lossy().to_string());
                    results.push(DetectedProcess {
                        pid: pid_u32,
                        tool: tool_name.to_string(),
                        tool_color: color.to_string(),
                        comm: exe_path,
                        cwd,
                        args: String::new(),
                        start_time: process.start_time(),
                    });
                }
                continue;
            }

            // Check if it's a script interpreter running an AI tool
            if is_interpreter(&exe_path) {
                let args: String = process.cmd().iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some((tool_name, color)) = match_tool(&args) {
                    if seen_pids.insert(pid_u32) {
                        let cwd = process.cwd().map(|p| p.to_string_lossy().to_string());
                        results.push(DetectedProcess {
                            pid: pid_u32,
                            tool: tool_name.to_string(),
                            tool_color: color.to_string(),
                            comm: args.clone(),
                            cwd,
                            args,
                            start_time: process.start_time(),
                        });
                    }
                }
            }
        }
        results
    }

    pub fn get_pid_start_time(pid: u32) -> Option<u64> {
        let s = System::new_all();
        s.process(sysinfo::Pid::from_u32(pid)).map(|p| p.start_time())
    }
}

// Re-export platform implementations
#[cfg(target_os = "macos")]
pub use macos::{scan_processes, get_pid_start_time};
#[cfg(not(target_os = "macos"))]
pub use generic::{scan_processes, get_pid_start_time};

// is_pid_alive: Unix uses kill(0), Windows uses sysinfo
#[cfg(unix)]
pub fn is_pid_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
pub fn is_pid_alive(pid: u32) -> bool {
    use sysinfo::System;
    let s = System::new_all();
    s.process(sysinfo::Pid::from_u32(pid)).is_some()
}
