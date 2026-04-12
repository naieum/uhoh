import type { TerminalTool } from "../lib/types";

interface RestoreBarProps {
  tools: TerminalTool[];
  selectedTool: string;
  onToolChange: (tool: string) => void;
}

export function RestoreBar({ tools, selectedTool, onToolChange }: RestoreBarProps) {
  const availableTools = tools.filter((t) => t.available);
  if (availableTools.length <= 1) return null;

  return (
    <div className="px-4 py-2 flex items-center gap-2">
      <span style={{ fontSize: 11, color: "rgba(255,255,255,0.25)" }}>Open in</span>
      <select
        value={selectedTool}
        onChange={(e) => onToolChange(e.target.value)}
        style={{
          fontSize: 11,
          background: "rgba(255,255,255,0.06)",
          color: "rgba(255,255,255,0.7)",
          border: "1px solid rgba(255,255,255,0.08)",
          borderRadius: 5,
          padding: "3px 8px",
          outline: "none",
        }}
      >
        {availableTools.map((tool) => (
          <option key={tool.id} value={tool.id}>{tool.name}</option>
        ))}
      </select>
    </div>
  );
}
