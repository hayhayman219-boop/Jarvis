import { useEffect, useState } from "react";
import { listNotionPages, readNotionPage, type NotionPage } from "../../lib/apiClient";

// A pop-up screen listing the Notion pages shared with Jarvis's integration;
// clicking a page loads and shows its text. Styled to match the HUD panels.
export function NotionViewer({ onClose }: { onClose: () => void }) {
  const [pages, setPages] = useState<NotionPage[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [active, setActive] = useState<NotionPage | null>(null);
  const [content, setContent] = useState<string>("");
  const [contentLoading, setContentLoading] = useState(false);

  useEffect(() => {
    listNotionPages()
      .then((p) => setPages(p))
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  function open(page: NotionPage) {
    setActive(page);
    setContent("");
    setContentLoading(true);
    readNotionPage(page.id)
      .then(setContent)
      .catch((e) => setContent(`[Could not load page: ${e}]`))
      .finally(() => setContentLoading(false));
  }

  return (
    <div
      style={{
        position: "absolute",
        top: "50%",
        left: "50%",
        transform: "translate(-50%, -50%)",
        width: 720,
        maxWidth: "92vw",
        height: 560,
        maxHeight: "88vh",
        display: "flex",
        background: "rgba(4,8,14,0.94)",
        border: "1px solid var(--jarvis-cyan-dim)",
        borderRadius: 6,
        overflow: "hidden",
        zIndex: 20,
      }}
    >
      {/* page list */}
      <div style={{ width: 240, borderRight: "1px solid var(--jarvis-cyan-dim)", display: "flex", flexDirection: "column" }}>
        <div style={{ padding: "12px 14px", borderBottom: "1px solid var(--jarvis-cyan-dim)" }}>
          <span className="jarvis-label">Notion</span>
        </div>
        <div style={{ overflowY: "auto", flex: 1 }}>
          {loading && <div style={{ padding: 14, fontSize: 13 }}>Loading pages…</div>}
          {error && <div style={{ padding: 14, fontSize: 13, color: "var(--jarvis-red)" }}>{error}</div>}
          {!loading && !error && pages.length === 0 && (
            <div style={{ padding: 14, fontSize: 12, lineHeight: 1.5, opacity: 0.8 }}>
              No pages found. In Notion, share a page with your integration
              (page ••• menu → Connections), then reopen this.
            </div>
          )}
          {pages.map((p) => (
            <button
              key={p.id}
              onClick={() => open(p)}
              style={{
                display: "block",
                width: "100%",
                textAlign: "left",
                padding: "10px 14px",
                background: active?.id === p.id ? "rgba(47,212,255,0.12)" : "transparent",
                border: "none",
                borderBottom: "1px solid rgba(47,212,255,0.08)",
                cursor: "pointer",
                fontSize: 13,
              }}
            >
              {p.title}
            </button>
          ))}
        </div>
      </div>

      {/* page content */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", minWidth: 0 }}>
        <div
          style={{
            padding: "12px 16px",
            borderBottom: "1px solid var(--jarvis-cyan-dim)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
          }}
        >
          <span style={{ fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
            {active ? active.title : "Select a page"}
          </span>
          <button
            onClick={onClose}
            aria-label="Close Notion viewer"
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
        <div style={{ padding: 20, overflowY: "auto", flex: 1, whiteSpace: "pre-wrap", lineHeight: 1.55, fontSize: 14 }}>
          {contentLoading ? "Loading…" : active ? content : "Choose a page from the left to read it."}
        </div>
      </div>
    </div>
  );
}
