interface StatusHeaderProps {
  activeCount: number;
  crashedCount: number;
}

export function StatusHeader({ activeCount, crashedCount }: StatusHeaderProps) {
  if (crashedCount > 0) {
    return (
      <div className="px-4 py-3 animate-in" style={{ background: "rgba(239, 68, 54, 0.08)" }}>
        <div className="flex items-center gap-2.5">
          <div className="w-2.5 h-2.5 rounded-full bg-red-400 pulse-glow" />
          <span style={{ color: "rgba(248, 113, 113, 0.95)", fontSize: 13, fontWeight: 600 }}>
            {crashedCount} crashed session{crashedCount !== 1 ? "s" : ""}
          </span>
        </div>
        {activeCount > 0 && (
          <p style={{ fontSize: 11, color: "rgba(255,255,255,0.35)", marginTop: 2, marginLeft: 18 }}>
            {activeCount} still active
          </p>
        )}
      </div>
    );
  }

  if (activeCount > 0) {
    return (
      <div className="px-4 py-3">
        <div className="flex items-center gap-2.5">
          <div className="w-2 h-2 rounded-full pulse-glow" style={{ background: "#34D399" }} />
          <span style={{ fontSize: 13, fontWeight: 600, color: "rgba(255,255,255,0.85)" }}>
            {activeCount} active session{activeCount !== 1 ? "s" : ""}
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className="px-4 py-4">
      <div className="flex flex-col items-center gap-1.5 py-2">
        <div className="w-2 h-2 rounded-full" style={{ background: "rgba(255,255,255,0.15)" }} />
        <span style={{ fontSize: 12, color: "rgba(255,255,255,0.35)" }}>No sessions detected</span>
        <span style={{ fontSize: 11, color: "rgba(255,255,255,0.2)" }}>
          Start an AI coding tool to begin tracking
        </span>
      </div>
    </div>
  );
}
