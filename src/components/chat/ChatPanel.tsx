import { useEffect, useMemo, useRef } from "react";
import { useChatStore } from "../../state/chatStore";
import { useSettingsStore } from "../../state/settingsStore";
import { interruptSpeaking } from "../../state/voiceStore";
import { MessageBubble } from "./MessageBubble";
import { ChatInput } from "./ChatInput";

// One AI's conversation: shows only messages tagged with `aiId`, and every
// send/stop is scoped to that AI. Two of these (Jarvis + Hacks) run side by
// side in their own draggable windows.
export function ChatPanel({ aiId, accent, name }: { aiId: string; accent: string; name: string }) {
  const allMessages = useChatStore((s) => s.messages);
  const sendMessage = useChatStore((s) => s.sendMessage);
  const status = useChatStore((s) => s.status);
  const streamingAiId = useChatStore((s) => s.streamingAiId);
  const model = useSettingsStore((s) => s.model);
  const scrollRef = useRef<HTMLDivElement>(null);

  const messages = useMemo(
    () => allMessages.filter((m) => m.aiId === aiId && m.role !== "system"),
    [allMessages, aiId],
  );

  // Only one reply generates at a time (tokens stream over a single channel),
  // so block input while any AI is busy — the other panel would interleave.
  const busy = streamingAiId !== null;
  const speakingHere = status === "speaking" && streamingAiId === aiId;

  useEffect(() => {
    scrollRef.current?.scrollTo({ top: scrollRef.current.scrollHeight });
  }, [messages]);

  return (
    <div
      className="jarvis-panel"
      style={{ display: "flex", flexDirection: "column", flex: 1, minHeight: 0, border: "none", borderRadius: 0 }}
    >
      <div
        ref={scrollRef}
        style={{
          flex: 1,
          overflowY: "auto",
          display: "flex",
          flexDirection: "column",
          gap: 10,
          padding: 16,
        }}
      >
        {messages.length === 0 && (
          <span className="jarvis-label">Awaiting your command.</span>
        )}
        {messages.map((m, i) => (
          <MessageBubble key={i} message={m} accent={accent} />
        ))}
        {speakingHere && (
          <button
            onClick={interruptSpeaking}
            style={{
              alignSelf: "flex-start",
              background: "transparent",
              border: `1px solid ${accent}`,
              color: accent,
              borderRadius: 4,
              padding: "4px 10px",
              fontSize: "0.75rem",
              cursor: "pointer",
            }}
          >
            ⏹ Stop speaking
          </button>
        )}
      </div>
      <ChatInput
        disabled={!model || busy}
        placeholder={!model ? "Select a model in Settings first..." : `Speak to ${name}...`}
        onSend={(text) => model && sendMessage(model, text, aiId)}
      />
    </div>
  );
}
