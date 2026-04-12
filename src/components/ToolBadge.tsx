interface ToolBadgeProps {
  tool: string;
  color: string;
}

export function ToolBadge({ tool, color }: ToolBadgeProps) {
  return (
    <span
      className="tool-badge"
      style={{
        background: `${color}18`,
        color: color,
        border: `1px solid ${color}25`,
      }}
    >
      {tool}
    </span>
  );
}
