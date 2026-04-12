import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { getSessions, getCrashes } from "../lib/tauri";
import type { TrackedSession, CrashEvent } from "../lib/types";

export function useSessions() {
  const [sessions, setSessions] = useState<TrackedSession[]>([]);
  const [crashes, setCrashes] = useState<CrashEvent[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [s, c] = await Promise.all([getSessions(), getCrashes()]);
      setSessions(s);
      setCrashes(c);
    } catch (err) {
      console.error("Failed to fetch sessions:", err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();

    // Purely event-driven - no polling. Backend pushes updates via events.
    const unlisten1 = listen("sessions-updated", () => refresh());
    const unlisten2 = listen("crash-detected", () => refresh());

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
    };
  }, [refresh]);

  const activeSessions = sessions.filter((s) => s.status === "Active");
  const crashedSessions = sessions.filter((s) => s.status === "Crashed");
  const endedSessions = sessions.filter((s) => s.status === "Ended" || s.status === "Recovered");

  return { sessions, activeSessions, crashedSessions, endedSessions, crashes, loading, refresh };
}
