import { useState } from "react";
import { PopupScreen } from "../hud/PopupScreen";
import { useComicSearchStore } from "../../state/comicSearchStore";
import { openUrls } from "../../lib/apiClient";

// Cover-art grid backed by Comic Vine. Populated by the chat intent (Hacks
// looking up a comic) or by typing in the search box here. Clicking a cover
// opens the issue's Comic Vine page in Chrome.
export function ComicCovers({ onClose, index }: { onClose: () => void; index: number }) {
  const { query, results, loading, error, search } = useComicSearchStore();
  const [text, setText] = useState(query);

  return (
    <PopupScreen title="Comic Covers" onClose={onClose} index={index} width={620} height={620}>
      <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 12, height: "100%", boxSizing: "border-box" }}>
        <form
          style={{ display: "flex", gap: 8 }}
          onSubmit={(e) => {
            e.preventDefault();
            void search(text);
          }}
        >
          <input
            value={text}
            onChange={(e) => setText(e.currentTarget.value)}
            placeholder="Search comics — e.g. Tales of Suspense 39"
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
            type="submit"
            style={{
              background: "transparent",
              border: "1px solid var(--jarvis-gold)",
              color: "var(--jarvis-gold)",
              borderRadius: 4,
              padding: "0 14px",
              cursor: "pointer",
            }}
          >
            Search
          </button>
        </form>

        {loading && <span className="jarvis-label">Searching Comic Vine…</span>}
        {error && !loading && <span style={{ color: "var(--jarvis-text-dim)", fontSize: 13 }}>{error}</span>}

        <div
          style={{
            flex: 1,
            overflowY: "auto",
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(130px, 1fr))",
            gap: 12,
            alignContent: "start",
          }}
        >
          {results.map((c, i) => (
            <div
              key={i}
              onClick={() => c.detail_url && void openUrls([c.detail_url]).catch(() => {})}
              title={c.detail_url ? "Open on Comic Vine" : ""}
              style={{ cursor: c.detail_url ? "pointer" : "default", display: "flex", flexDirection: "column", gap: 4 }}
            >
              <img
                src={c.image_url}
                alt={c.name || c.volume}
                loading="lazy"
                style={{ width: "100%", borderRadius: 4, border: "1px solid var(--jarvis-cyan-dim)", aspectRatio: "2/3", objectFit: "cover" }}
              />
              <div style={{ fontSize: 12, color: "var(--jarvis-text)", lineHeight: 1.2 }}>
                {c.volume}
                {c.issue_number ? ` #${c.issue_number}` : ""}
              </div>
              <div style={{ fontSize: 10, color: "var(--jarvis-text-dim)" }}>
                {c.cover_date || c.name}
              </div>
            </div>
          ))}
        </div>
      </div>
    </PopupScreen>
  );
}
