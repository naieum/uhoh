import type { TrackedSession } from "../lib/types";
import { SessionCard } from "./SessionCard";

interface SessionListProps {
  sessions: TrackedSession[];
}

export function SessionList({ sessions }: SessionListProps) {
  if (sessions.length === 0) return null;

  const sorted = [...sessions].sort((a, b) => {
    if (a.tool !== b.tool) return a.tool.localeCompare(b.tool);
    return b.started_at - a.started_at;
  });

  return (
    <div className="animate-in">
      {sorted.map((session) => (
        <SessionCard key={session.id} session={session} />
      ))}
    </div>
  );
}
