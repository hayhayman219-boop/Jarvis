import type { ChatMessage } from "../../lib/types";
import { MarkdownText } from "./MarkdownText";

export function MessageBubble({ message, accent }: { message: ChatMessage; accent?: string }) {
  const isUser = message.role === "user";
  const assistantBorder = accent ?? "var(--jarvis-cyan-dim)";
  return (
    <div
      style={{
        alignSelf: isUser ? "flex-end" : "flex-start",
        maxWidth: "75%",
        padding: "8px 12px",
        borderRadius: 4,
        border: `1px solid ${isUser ? "var(--jarvis-gold)" : assistantBorder}`,
        color: isUser ? "var(--jarvis-gold)" : "var(--jarvis-text)",
        background: "rgba(255,255,255,0.02)",
        whiteSpace: "pre-wrap",
        // Long words / URLs must wrap, or they push the bubble past the
        // window edge (Hacks' link-heavy replies were overflowing).
        overflowWrap: "anywhere",
        wordBreak: "break-word",
        minWidth: 0,
      }}
    >
      {isUser ? message.content : <MarkdownText text={message.content} />}
    </div>
  );
}
