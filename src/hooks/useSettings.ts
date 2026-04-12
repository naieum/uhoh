import { useState, useEffect } from "react";
import { getAvailableTools } from "../lib/tauri";
import type { TerminalTool } from "../lib/types";

export function useSettings() {
  const [tools, setTools] = useState<TerminalTool[]>([]);
  const [selectedTool, setSelectedTool] = useState("terminal");

  useEffect(() => {
    getAvailableTools().then((t) => {
      setTools(t);
      // Default to first available tool
      const available = t.find((tool) => tool.available);
      if (available) {
        setSelectedTool(available.id);
      }
    });
  }, []);

  return { tools, selectedTool, setSelectedTool };
}
