import { useState } from "react";
import type { TrackedSession } from "../lib/types";
import { SessionCard } from "./SessionCard";

interface CrashPanelProps {
  sessions: TrackedSession[];
  onRestore: (sessionId: string) => void;
  onRestoreAll: () => void;
}

export function CrashPanel({ sessions, onRestore, onRestoreAll }: CrashPanelProps) {
  const [selected, setSelected] = useState<Set<string>>(
    new Set(sessions.map((s) => s.id))
  );

  if (sessions.length === 0) return null;

  const toggleSession = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleAll = () => {
    if (selected.size === sessions.length) setSelected(new Set());
    else setSelected(new Set(sessions.map((s) => s.id)));
  };

  const sorted = [...sessions].sort((a, b) => {
    if (a.tool !== b.tool) return a.tool.localeCompare(b.tool);
    return a.project_name.localeCompare(b.project_name);
  });

  return (
    <div className="animate-in">
      {sorted.map((session) => (
        <SessionCard
          key={session.id}
          session={session}
          selectable
          selected={selected.has(session.id)}
          onToggle={() => toggleSession(session.id)}
          onRestore={() => onRestore(session.id)}
        />
      ))}

      <div className="px-4 py-2.5 flex items-center gap-2">
        <button className="btn-ghost" onClick={toggleAll}>
          {selected.size === sessions.length ? "Deselect all" : "Select all"}
        </button>
        <div className="flex-1" />
        <button
          className="btn-primary"
          onClick={onRestoreAll}
          disabled={selected.size === 0}
        >
          Restore All ({selected.size})
        </button>
      </div>
    </div>
  );
}
