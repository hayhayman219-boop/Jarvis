import { useEffect, useState, type ReactNode } from "react";
import { useChatStore } from "../../state/chatStore";
import { ChatPanel } from "./ChatPanel";
import { DraggableWindow } from "./DraggableWindow";
import { ALL_AIS, type AiPersona } from "../../data/subAIs";

// The chat workspace: one movable window per AI (Jarvis, Hacks, …), each with
// its own conversation. Escape closes the whole workspace.
export function ChatOverlay({ onClose }: { onClose: () => void }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    // Layer that lets clicks pass through the gaps; only the windows capture.
    <div style={{ position: "absolute", inset: 0, pointerEvents: "none" }}>
      {ALL_AIS.map((ai, i) => (
        <div key={ai.id} style={{ pointerEvents: "auto" }}>
          <ChatWindow
            ai={ai}
            defaultPos={{ x: 40 + i * 460, y: 60 + i * 40 }}
            onClose={onClose}
          />
        </div>
      ))}
    </div>
  );
}

function HeaderButton({
  accent,
  onClick,
  title,
  children,
}: {
  accent: string;
  onClick: () => void;
  title: string;
  children: ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      style={{
        background: "transparent",
        border: `1px solid ${accent}66`,
        color: accent,
        borderRadius: 4,
        padding: "0 8px",
        height: 22,
        fontSize: "0.7rem",
        cursor: "pointer",
      }}
    >
      {children}
    </button>
  );
}

function ChatWindow({
  ai,
  defaultPos,
  onClose,
}: {
  ai: AiPersona;
  defaultPos: { x: number; y: number };
  onClose: () => void;
}) {
  const [minimized, setMinimized] = useState(false);
  const clearHistory = useChatStore((s) => s.clearHistory);

  return (
    <DraggableWindow
      storageKey={`chat-pos-${ai.id}`}
      defaultPos={defaultPos}
      width={430}
      height={minimized ? 40 : 580}
      accent={ai.accent}
      title={
        <>
          {ai.name}
          <span style={{ opacity: 0.55, fontWeight: 400 }}> · {ai.tagline}</span>
        </>
      }
      headerRight={
        <>
          <HeaderButton
            accent={ai.accent}
            title={`Clear ${ai.name}'s chat`}
            onClick={() => confirm(`Clear ${ai.name}'s chat?`) && clearHistory(ai.id)}
          >
            Clear
          </HeaderButton>
          <HeaderButton
            accent={ai.accent}
            title={minimized ? "Expand" : "Minimize"}
            onClick={() => setMinimized((m) => !m)}
          >
            {minimized ? "▸" : "▾"}
          </HeaderButton>
          <HeaderButton accent={ai.accent} title="Close chat (Esc)" onClick={onClose}>
            ✕
          </HeaderButton>
        </>
      }
    >
      {minimized ? <div /> : <ChatPanel aiId={ai.id} accent={ai.accent} name={ai.name} />}
    </DraggableWindow>
  );
}
