import type { TrackedSession } from "../lib/types";
import { ToolBadge } from "./ToolBadge";

interface SessionCardProps {
  session: TrackedSession;
  selectable?: boolean;
  selected?: boolean;
  onToggle?: () => void;
  onRestore?: () => void;
}

function formatUptime(startedAt: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - startedAt;
  if (diff < 0) return "now";
  if (diff < 60) return `${diff}s`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  const h = Math.floor(diff / 3600);
  const m = Math.floor((diff % 3600) / 60);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}

function truncate(str: string, max: number): string {
  if (str.length <= max) return str;
  return str.slice(0, max - 1) + "\u2026";
}

export function SessionCard({
  session,
  selectable,
  selected,
  onToggle,
  onRestore,
}: SessionCardProps) {
  const summary =
    session.metadata.summary ||
    session.metadata.session_name ||
    (session.metadata.first_prompt
      ? truncate(session.metadata.first_prompt, 45)
      : null);

  const branch = session.metadata.git_branch;
  const isCrashed = session.status === "Crashed";

  const handleDragStart = (e: React.DragEvent) => {
    // Set the resume command as plain text - drops into any terminal
    e.dataTransfer.setData("text/plain", session.resume_cmd);
    e.dataTransfer.effectAllowed = "copy";
  };

  return (
    <div
      draggable
      onDragStart={handleDragStart}
      className={`session-card flex items-start gap-2.5 px-4 py-2.5 draggable-card ${
        isCrashed ? "" : ""
      }`}
      style={
        isCrashed ? { background: "rgba(239, 68, 54, 0.04)" } : undefined
      }
      title="Drag into a terminal to resume"
      onClick={selectable ? onToggle : undefined}
    >
      {selectable && (
        <div className="pt-0.5">
          <input
            type="checkbox"
            checked={selected}
            onChange={onToggle}
            style={{ accentColor: "#8B5CF6", width: 14, height: 14 }}
          />
        </div>
      )}

      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2">
          <ToolBadge tool={session.tool} color={session.tool_color} />
          <span
            className="truncate"
            style={{
              fontSize: 13,
              fontWeight: 500,
              color: "rgba(255,255,255,0.88)",
            }}
          >
            {session.project_name}
          </span>
          {branch && (
            <span
              className="truncate"
              style={{ fontSize: 11, color: "rgba(255,255,255,0.25)" }}
            >
              {branch}
            </span>
          )}
        </div>

        {summary && (
          <p
            className="truncate mt-0.5"
            style={{ fontSize: 11, color: "rgba(255,255,255,0.4)" }}
          >
            {summary}
          </p>
        )}
      </div>

      <div className="flex items-center gap-2 shrink-0 pt-0.5">
        {isCrashed && onRestore ? (
          <button className="btn-primary" onClick={(e) => { e.stopPropagation(); onRestore(); }}>
            Restore
          </button>
        ) : (
          <span style={{ fontSize: 11, color: "rgba(255,255,255,0.2)", fontVariantNumeric: "tabular-nums" }}>
            {formatUptime(session.started_at)}
          </span>
        )}
      </div>
    </div>
  );
}
