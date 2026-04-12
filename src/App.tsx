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

function SessionRow({ session, selecting, selected, onSelect, onRestore }: {
  session: TrackedSession;
  selecting: boolean;
  selected: boolean;
  onSelect: () => void;
  onRestore: () => void;
}) {
  const summary = session.metadata.summary || session.metadata.session_name ||
    (session.metadata.first_prompt ? truncate(session.metadata.first_prompt, 40) : null);
  const isActive = session.status === "Active";

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
      <div className="w-1.5 h-1.5 rounded-full shrink-0" style={{
        background: session.status === "Crashed" ? "#F87171" : isActive ? "#34D399" : "#475569"
      }} />
      <div className="flex-1 min-w-0">
        <span className="truncate block" style={{ fontSize: 12, fontWeight: 500, opacity: isActive ? 1 : 0.5 }}>
          {session.project_name}
        </span>
        {summary && (
          <span className="truncate block" style={{ fontSize: 10, color: "rgba(255,255,255,0.25)", marginTop: 1 }}>
            {summary}
          </span>
        )}
      </div>
      {!selecting && (
        !isActive ? (
          <button className="btn-ghost" onClick={(e) => { e.stopPropagation(); onRestore(); }}>
            Open
          </button>
        ) : (
          <span style={{ fontSize: 10, color: "rgba(255,255,255,0.15)", fontVariantNumeric: "tabular-nums" }}>
            {formatTime(session.started_at)}
          </span>
        )
      )}
    </div>
  );
}

interface ToolGroup {
  tool: string;
  color: string;
  sessions: TrackedSession[];
}

function ToolSection({ group, selecting, selectedIds, onSelect, onRestore, onOpenAll, onToggle }: {
  group: ToolGroup;
  selecting: boolean;
  selectedIds: Set<string>;
  onSelect: (id: string) => void;
  onRestore: (id: string) => void;
  onOpenAll: (ids: string[]) => void;
  onToggle?: () => void;
}) {
  const [open, setOpen] = useState(false);
  const toggle = () => { setOpen(!open); setTimeout(() => onToggle?.(), 50); };
  const activeCount = group.sessions.filter(s => s.status === "Active").length;
  const restorableIds = group.sessions.filter(s => s.status !== "Active").map(s => s.id);

  return (
    <div className="card">
      <div className="section-header" onClick={toggle}>
        <span className={`chevron ${open ? "open" : ""}`}>&#9654;</span>
        <ToolBadge tool={group.tool} color={group.color} />
        <span style={{ fontSize: 11, color: "rgba(255,255,255,0.2)" }}>
          {group.sessions.length}
        </span>
        {activeCount > 0 && (
          <div className="w-1.5 h-1.5 rounded-full pulse" style={{ background: "#34D399" }} />
        )}
        <div className="flex-1" />
        {!selecting && restorableIds.length > 1 && open && (
          <button
            className="btn-ghost"
            onClick={(e) => { e.stopPropagation(); onOpenAll(restorableIds); }}
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

  // Multi-select mode for tmux grid
  const [selecting, setSelecting] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

  const fitWindow = useCallback(() => {
    if (!containerRef.current) return;
    const h = containerRef.current.scrollHeight;
    const clamped = Math.max(80, Math.min(h + 24, 600));
    getCurrentWindow().setSize(new LogicalSize(320, clamped)).catch(() => {});
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => { if (e.key === "Escape") { if (selecting) { setSelecting(false); setSelectedIds(new Set()); } else { window.close(); } } };
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

  const groups = useMemo<ToolGroup[]>(() => {
    const map = new Map<string, ToolGroup>();
    for (const s of sessions) {
      if (!map.has(s.tool)) map.set(s.tool, { tool: s.tool, color: s.tool_color, sessions: [] });
      map.get(s.tool)!.sessions.push(s);
    }
    const statusOrder: Record<string, number> = { Crashed: 0, Active: 1, Ended: 2, Recovered: 3 };
    for (const group of map.values()) {
      group.sessions.sort((a, b) => {
        const so = (statusOrder[a.status] ?? 9) - (statusOrder[b.status] ?? 9);
        return so !== 0 ? so : b.last_seen - a.last_seen;
      });
    }
    return [...map.values()].sort((a, b) => {
      const aActive = a.sessions.some(s => s.status === "Active" || s.status === "Crashed");
      const bActive = b.sessions.some(s => s.status === "Active" || s.status === "Crashed");
      if (aActive && !bActive) return -1;
      if (!aActive && bActive) return 1;
      return a.tool.localeCompare(b.tool);
    });
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

      {/* Tool groups */}
      <div className="flex flex-col gap-2">
        {groups.map((group) => (
          <ToolSection
            key={group.tool}
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
