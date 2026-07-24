import { useState } from "react";
import { manualStartListening, manualStopAndTranscribe, useVoiceStore } from "../../state/voiceStore";

export function ChatInput({
  disabled,
  onSend,
  placeholder,
}: {
  disabled: boolean;
  onSend: (text: string) => void;
  placeholder?: string;
}) {
  const [text, setText] = useState("");
  const isRecording = useVoiceStore((s) => s.isRecording);
  const error = useVoiceStore((s) => s.error);

  function submit() {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setText("");
  }

  async function handleMicUp() {
    const transcript = await manualStopAndTranscribe();
    if (transcript) onSend(transcript);
  }

  return (
    <div style={{ padding: 12 }}>
      <form
        style={{ display: "flex", gap: 8 }}
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <input
          value={text}
          disabled={disabled}
          onChange={(e) => setText(e.currentTarget.value)}
          placeholder={placeholder ?? (disabled ? "Select a model in Settings first..." : "Speak to Jarvis...")}
          style={{
            flex: 1,
            background: "var(--jarvis-bg)",
            border: "1px solid var(--jarvis-cyan-dim)",
            color: "var(--jarvis-text)",
            padding: "8px 10px",
            borderRadius: 4,
          }}
        />
        <button
          type="button"
          disabled={disabled}
          onMouseDown={manualStartListening}
          onMouseUp={handleMicUp}
          onMouseLeave={() => isRecording && handleMicUp()}
          title='Hold to talk, or just say "Jarvis" followed by your request'
          style={{
            background: isRecording ? "var(--jarvis-gold)" : "transparent",
            border: "1px solid var(--jarvis-gold)",
            color: isRecording ? "var(--jarvis-bg)" : "var(--jarvis-gold)",
            padding: "8px 12px",
            borderRadius: 4,
            cursor: disabled ? "not-allowed" : "pointer",
          }}
        >
          {isRecording ? "Listening..." : "🎙"}
        </button>
        <button
          type="submit"
          disabled={disabled}
          style={{
            background: "transparent",
            border: "1px solid var(--jarvis-cyan)",
            color: "var(--jarvis-cyan)",
            padding: "8px 16px",
            borderRadius: 4,
            cursor: disabled ? "not-allowed" : "pointer",
          }}
        >
          Send
        </button>
      </form>
      {error && (
        <div style={{ color: "var(--jarvis-red)", fontSize: "0.75rem", marginTop: 4 }}>{error}</div>
      )}
    </div>
  );
}
