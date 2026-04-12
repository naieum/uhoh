export interface SessionMeta {
  summary: string | null;
  first_prompt: string | null;
  message_count: number | null;
  git_branch: string | null;
  session_id: string | null;
  session_name: string | null;
}

export interface TrackedSession {
  id: string;
  tool: string;
  tool_color: string;
  pid: number;
  cwd: string;
  project_name: string;
  started_at: number;
  last_seen: number;
  status: "Active" | "Crashed" | "Ended" | "Recovered";
  metadata: SessionMeta;
  resume_cmd: string;
  start_time: number;
  from_index: boolean;
}

export interface CrashEvent {
  id: string;
  detected_at: number;
  sessions: string[];
  dismissed: boolean;
}

export interface TerminalTool {
  id: string;
  name: string;
  available: boolean;
}
