import { invoke } from "@tauri-apps/api/core";
import type { TrackedSession, CrashEvent, TerminalTool } from "./types";

export async function getSessions(): Promise<TrackedSession[]> {
  return invoke("get_sessions");
}

export async function getCrashes(): Promise<CrashEvent[]> {
  return invoke("get_crashes");
}

export async function restoreSession(
  sessionId: string,
  tool: string
): Promise<void> {
  return invoke("restore_session", { sessionId, tool });
}

export async function restoreAll(tool: string): Promise<number> {
  return invoke("restore_all", { tool });
}

export async function dismissCrash(crashId: string): Promise<void> {
  return invoke("dismiss_crash", { crashId });
}

export async function openMultiple(
  sessionIds: string[],
  tool: string
): Promise<number> {
  return invoke("open_multiple", { sessionIds, tool });
}

export async function getAvailableTools(): Promise<TerminalTool[]> {
  return invoke("get_available_tools");
}

export async function quitApp(): Promise<void> {
  return invoke("quit_app");
}
