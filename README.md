# uhoh

Session monitor and recovery tool for AI coding assistants. Detects crashes, lets you restore sessions instantly.

## What it does

uhoh watches for running AI coding assistant processes across your system. When a session crashes or your terminal dies, uhoh catches it and lets you restore everything with one click.

**Supported tools:** Claude Code, Gemini CLI, Codex, Aider, Goose, Kimi, OpenCode

**Supported terminals:**
- **macOS:** Terminal.app, iTerm2, Warp, Ghostty, Alacritty, Kitty, WezTerm, tmux, and more
- **Linux:** GNOME Terminal, Konsole, Kitty, Alacritty, WezTerm, Foot, xterm, tmux
- **Windows:** Windows Terminal, PowerShell, Command Prompt

## Install

Download the latest release from [Releases](https://github.com/naieum/uhoh/releases).

- **macOS:** Download the `.dmg`, open it, drag uhoh to Applications
- **Linux / Windows:** Build from source (see below)

### Building from source

```bash
npm install
npm run tauri build
```

Bundled app will be at `src-tauri/target/release/bundle/`.

**Requirements:** Node.js 18+, Rust stable. On macOS: Xcode Command Line Tools. On Linux: `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, and other [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

## How it works

- **Layer 1:** kqueue for instant process death detection (macOS) or periodic polling (Linux/Windows)
- **Layer 2:** File system watchers for session state changes
- **Layer 3:** Periodic process scanning as fallback

Uses native macOS APIs (libproc) for fast process scanning when available, sysinfo crate on other platforms. Frontend is React + Tailwind with a frosted-glass tray popup.

## License

AGPL-3.0
