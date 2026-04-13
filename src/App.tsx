import { useEffect, useMemo, useState, useRef, useCallback } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useSessions } from "./hooks/useSessions";
import { useSettings } from "./hooks/useSettings";
import { restoreSession, openMultiple, quitApp } from "./lib/tauri";
import { ToolBadge } from "./components/ToolBadge";
import type { TrackedSession } from "./lib/types";

function formatTime(ts: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - ts;
  if (diff < 3600) return "new";
  return `${Math.floor(diff / 3600)}h`;
}

function truncate(s: string, n: number) {
  return s.length > n ? s.slice(0, n - 1) + "\u2026" : s;
}

function SessionRow({ session, selecting, selected, onSelect, onRestore, inProjectGroup }: {
  session: TrackedSession;
  selecting: boolean;
  selected: boolean;
  onSelect: () => void;
  onRestore: () => void;
  inProjectGroup?: boolean;
}) {
  const summary = session.metadata.summary || session.metadata.session_name ||
    (session.metadata.first_prompt ? truncate(session.metadata.first_prompt, 40) : null);
  const isActive = session.status === "Active";
  const isCrashed = session.status === "Crashed";

  return (
    <div
      className="session-row flex items-center gap-2.5"
      draggable={!selecting}
      onDragStart={(e) => {
        e.dataTransfer.setData("text/plain", session.resume_cmd);
        e.dataTransfer.effectAllowed = "copy";
      }}
      onClick={selecting ? onSelect : undefined}
      style={{ cursor: selecting ? "pointer" : "grab" }}
      title={session.resume_cmd}
    >
      {selecting && (
        <input
          type="checkbox"
          checked={selected}
          onChange={(e) => { e.stopPropagation(); onSelect(); }}
          onClick={(e) => e.stopPropagation()}
          style={{ accentColor: "#8B5CF6", width: 13, height: 13, cursor: "pointer" }}
        />
      )}
      <div className={`w-1.5 h-1.5 rounded-full shrink-0${isActive ? " pulse" : ""}`} style={{
        background: isCrashed ? "#F87171" : isActive ? "#34D399" : "#475569"
      }} />
      <ToolBadge tool={session.tool} color={session.tool_color} />
      <div className="flex-1 min-w-0">
        <span className="truncate block" style={{ fontSize: 12, fontWeight: 500, opacity: isActive || isCrashed ? 1 : 0.5 }}>
          {inProjectGroup ? (summary || session.project_name) : session.project_name}
        </span>
        {!inProjectGroup && summary && (
          <span className="truncate block" style={{ fontSize: 10, color: "rgba(255,255,255,0.25)", marginTop: 1 }}>
            {summary}
          </span>
        )}
      </div>
      {!selecting && (
        isActive ? (
          <span style={{ fontSize: 10, color: "rgba(255,255,255,0.15)", fontVariantNumeric: "tabular-nums" }}>
            {formatTime(session.started_at)}
          </span>
        ) : (
          <button className="btn-ghost" onClick={(e) => { e.stopPropagation(); onRestore(); }}>
            Open
          </button>
        )
      )}
    </div>
  );
}

interface ProjectGroup {
  project: string;
  cwd: string;
  sessions: TrackedSession[];
}

function ProjectSection({ group, selecting, selectedIds, onSelect, onRestore, onOpenAll, onToggle }: {
  group: ProjectGroup;
  selecting: boolean;
  selectedIds: Set<string>;
  onSelect: (id: string) => void;
  onRestore: (id: string) => void;
  onOpenAll: (ids: string[]) => void;
  onToggle?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const toggle = () => { setOpen(!open); setTimeout(() => onToggle?.(), 50); };
  const allIds = group.sessions.map(s => s.id);

  return (
    <div className="card">
      <div className="section-header" onClick={toggle}>
        <span className={`chevron ${open ? "open" : ""}`}>&#9654;</span>
        <span style={{ fontSize: 12, fontWeight: 500, color: "rgba(255,255,255,0.7)" }}>
          {group.project}
        </span>
        <span style={{ fontSize: 11, color: "rgba(255,255,255,0.2)" }}>
          {group.sessions.length}
        </span>
        <div className="flex-1" />
        {!selecting && allIds.length > 1 && open && (
          <button
            className="btn-ghost"
            onClick={(e) => { e.stopPropagation(); onOpenAll(allIds); }}
            style={{ fontSize: 10 }}
          >
            Open All
          </button>
        )}
      </div>
      {open && (
        <div className="section-content">
          {group.sessions.map((s) => (
            <SessionRow
              key={s.id}
              session={s}
              selecting={selecting}
              selected={selectedIds.has(s.id)}
              onSelect={() => onSelect(s.id)}
              onRestore={() => onRestore(s.id)}
              inProjectGroup
            />
          ))}
        </div>
      )}
    </div>
  );
}

export default function App() {
  const { sessions, loading } = useSessions();
  const { tools, selectedTool, setSelectedTool } = useSettings();
  const containerRef = useRef<HTMLDivElement>(null);

  const [selecting, setSelecting] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const fitWindow = useCallback(() => {
    if (!containerRef.current) return;
    const h = containerRef.current.scrollHeight;
    const clamped = Math.max(80, Math.min(h + 24, 600));
    getCurrentWindow().setSize(new LogicalSize(320, clamped)).catch(() => {});
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        if (selecting) { setSelecting(false); setSelectedIds(new Set()); }
        else { window.close(); }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selecting]);

  useEffect(() => { fitWindow(); }, [sessions, loading, selecting, fitWindow]);
  useEffect(() => {
    if (!containerRef.current) return;
    const observer = new ResizeObserver(() => fitWindow());
    observer.observe(containerRef.current);
    return () => observer.disconnect();
  }, [fitWindow]);

  const handleRestore = async (id: string) => {
    try { await restoreSession(id, selectedTool); } catch {}
  };
  const handleOpenAll = async (ids: string[]) => {
    try { await openMultiple(ids, selectedTool); } catch {}
  };
  const handleSelect = (id: string) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };
  const handleOpenSelected = async () => {
    if (selectedIds.size === 0) return;
    try {
      await openMultiple([...selectedIds], selectedTool);
      setSelecting(false);
      setSelectedIds(new Set());
    } catch {}
  };

  const { active, crashed, projectGroups } = useMemo(() => {
    const active: TrackedSession[] = [];
    const crashed: TrackedSession[] = [];
    const endedMap = new Map<string, ProjectGroup>();

    for (const s of sessions) {
      if (s.status === "Active") {
        active.push(s);
      } else if (s.status === "Crashed") {
        crashed.push(s);
      } else {
        const key = s.cwd;
        if (!endedMap.has(key)) {
          endedMap.set(key, { project: s.project_name, cwd: key, sessions: [] });
        }
        endedMap.get(key)!.sessions.push(s);
      }
    }

    active.sort((a, b) => b.started_at - a.started_at);
    crashed.sort((a, b) => b.last_seen - a.last_seen);

    for (const group of endedMap.values()) {
      group.sessions.sort((a, b) => b.last_seen - a.last_seen);
    }

    const projectGroups = [...endedMap.values()].sort((a, b) => {
      return (b.sessions[0]?.last_seen ?? 0) - (a.sessions[0]?.last_seen ?? 0);
    });

    return { active, crashed, projectGroups };
  }, [sessions]);

  if (loading) {
    return <div className="flex items-center justify-center h-full">
      <span style={{ fontSize: 12, color: "rgba(255,255,255,0.25)" }}>Loading...</span>
    </div>;
  }

  return (
    <div ref={containerRef} className="animate-in flex flex-col gap-2 p-3">
      {/* Header */}
      <div className="flex items-center justify-between px-1 pb-1">
        <span style={{ fontSize: 13, fontWeight: 700, color: "rgba(255,255,255,0.85)" }}>uhoh</span>
        <div className="flex items-center gap-2">
          {tools.filter(t => t.available).length > 1 && (
            <select
              value={selectedTool}
              onChange={(e) => setSelectedTool(e.target.value)}
              style={{
                fontSize: 10, background: "rgba(255,255,255,0.06)",
                color: "rgba(255,255,255,0.4)", border: "1px solid rgba(255,255,255,0.06)",
                borderRadius: 5, padding: "2px 6px", outline: "none",
              }}
            >
              {tools.filter(t => t.available).map(t => (
                <option key={t.id} value={t.id}>{t.name}</option>
              ))}
            </select>
          )}
          <button
            onClick={() => { if (selecting) { setSelecting(false); setSelectedIds(new Set()); } else { setSelecting(true); } }}
            className="btn-ghost"
            style={{
              fontSize: 10,
              background: selecting ? "rgba(139,92,246,0.2)" : undefined,
              color: selecting ? "rgba(139,92,246,0.9)" : undefined,
              borderColor: selecting ? "rgba(139,92,246,0.3)" : undefined,
            }}
          >
            {selecting ? "Cancel" : "Select"}
          </button>
          <button
            onClick={() => quitApp()}
            style={{ fontSize: 10, color: "rgba(255,255,255,0.15)", background: "none", border: "none", cursor: "pointer" }}
            onMouseEnter={(e) => (e.currentTarget.style.color = "rgba(255,255,255,0.4)")}
            onMouseLeave={(e) => (e.currentTarget.style.color = "rgba(255,255,255,0.15)")}
          >
            Quit
          </button>
        </div>
      </div>

      {/* Selection action bar */}
      {selecting && selectedIds.size > 0 && (
        <button
          className="btn-restore"
          onClick={handleOpenSelected}
          style={{ fontSize: 12 }}
        >
          Open {selectedIds.size} in {tools.find(t => t.id === selectedTool)?.name || selectedTool}
        </button>
      )}

      <div className="flex flex-col gap-2">
        {/* Active sessions - always visible at top */}
        {active.length > 0 && (
          <div className="card">
            <div style={{ padding: "8px 14px 2px", display: "flex", alignItems: "center", gap: 6 }}>
              <div className="w-1.5 h-1.5 rounded-full pulse" style={{ background: "#34D399" }} />
              <span style={{ fontSize: 10, fontWeight: 600, color: "rgba(255,255,255,0.3)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                Active
              </span>
            </div>
            {active.map((s) => (
              <SessionRow
                key={s.id}
                session={s}
                selecting={selecting}
                selected={selectedIds.has(s.id)}
                onSelect={() => handleSelect(s.id)}
                onRestore={() => handleRestore(s.id)}
              />
            ))}
            <div style={{ height: 4 }} />
          </div>
        )}

        {/* Crashed sessions - prominent between active and project groups */}
        {crashed.length > 0 && (
          <div className="card" style={{ borderColor: "rgba(248, 113, 113, 0.2)" }}>
            <div style={{ padding: "8px 14px 2px", display: "flex", alignItems: "center", gap: 6 }}>
              <div className="w-1.5 h-1.5 rounded-full" style={{ background: "#F87171" }} />
              <span style={{ fontSize: 10, fontWeight: 600, color: "rgba(248, 113, 113, 0.6)", textTransform: "uppercase", letterSpacing: "0.05em" }}>
                Crashed
              </span>
            </div>
            {crashed.map((s) => (
              <SessionRow
                key={s.id}
                session={s}
                selecting={selecting}
                selected={selectedIds.has(s.id)}
                onSelect={() => handleSelect(s.id)}
                onRestore={() => handleRestore(s.id)}
              />
            ))}
            <div style={{ height: 4 }} />
          </div>
        )}

        {/* Ended sessions grouped by project */}
        {projectGroups.map((group) => (
          <ProjectSection
            key={group.cwd}
            group={group}
            selecting={selecting}
            selectedIds={selectedIds}
            onSelect={handleSelect}
            onRestore={handleRestore}
            onOpenAll={handleOpenAll}
            onToggle={fitWindow}
          />
        ))}

        {sessions.length === 0 && (
          <div className="card flex items-center justify-center">
            <div className="text-center py-8">
              <div className="w-2.5 h-2.5 rounded-full mx-auto mb-2" style={{ background: "rgba(255,255,255,0.12)" }} />
              <p style={{ fontSize: 12, color: "rgba(255,255,255,0.3)" }}>No sessions</p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
