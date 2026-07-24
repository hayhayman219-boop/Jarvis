import type { ReactNode } from "react";

// Standard HUD pop-up window: a centered, dark, cyan-bordered panel with a
// title bar and close button. New screens wrap their content in this so they
// all share the same chrome; `index` cascades stacked windows so they don't
// perfectly overlap. (Screens with a bespoke layout — e.g. Notion's two-pane
// viewer — may render their own frame instead.)
export function PopupScreen({
  title,
  onClose,
  children,
  width = 560,
  height = 460,
  index = 0,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  width?: number;
  height?: number;
  index?: number;
}) {
  const offset = index * 28;
  return (
    <div
      style={{
        position: "absolute",
        top: `calc(50% + ${offset}px)`,
        left: `calc(50% + ${offset}px)`,
        transform: "translate(-50%, -50%)",
        width,
        maxWidth: "92vw",
        height,
        maxHeight: "88vh",
        display: "flex",
        flexDirection: "column",
        background: "rgba(4,8,14,0.94)",
        border: "1px solid var(--jarvis-cyan-dim)",
        borderRadius: 6,
        overflow: "hidden",
        zIndex: 20 + index,
      }}
    >
      <div
        style={{
          padding: "12px 16px",
          borderBottom: "1px solid var(--jarvis-cyan-dim)",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <span className="jarvis-label">{title}</span>
        <button
          onClick={onClose}
          aria-label={`Close ${title}`}
          style={{
            background: "transparent",
            border: "1px solid var(--jarvis-cyan-dim)",
            color: "var(--jarvis-cyan)",
            borderRadius: 4,
            width: 28,
            height: 28,
            cursor: "pointer",
          }}
        >
          ✕
        </button>
      </div>
      <div style={{ flex: 1, overflow: "auto", minHeight: 0 }}>{children}</div>
    </div>
  );
}
