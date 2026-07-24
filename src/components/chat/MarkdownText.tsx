import type { ReactNode } from "react";
import { openUrls } from "../../lib/apiClient";

// Minimal Markdown renderer for chat: headings (#, ##), bullet lists (-/*),
// **bold**, and [links](url) — enough for the Sub AIs' presentation replies.
// Links open in the external browser (Chrome) via the app's browser control.

function renderInline(text: string, keyBase: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const re = /\[([^\]]+)\]\((https?:\/\/[^)]+)\)|\*\*([^*]+)\*\*/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let i = 0;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) nodes.push(text.slice(last, m.index));
    if (m[2]) {
      const label = m[1];
      const url = m[2];
      nodes.push(
        <a
          key={`${keyBase}-${i}`}
          onClick={(e) => {
            e.preventDefault();
            void openUrls([url]).catch(() => {});
          }}
          style={{ color: "var(--jarvis-cyan)", textDecoration: "underline", cursor: "pointer" }}
        >
          {label}
        </a>,
      );
    } else if (m[3]) {
      nodes.push(<strong key={`${keyBase}-${i}`}>{m[3]}</strong>);
    }
    last = re.lastIndex;
    i++;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

export function MarkdownText({ text }: { text: string }) {
  const lines = text.split("\n");
  const blocks: ReactNode[] = [];
  let list: ReactNode[] = [];
  const flush = (k: string) => {
    if (list.length) {
      blocks.push(
        <ul key={k} style={{ margin: "4px 0", paddingLeft: 18 }}>
          {list}
        </ul>,
      );
      list = [];
    }
  };

  lines.forEach((line, idx) => {
    const li = line.match(/^\s*[-*]\s+(.*)/);
    if (li) {
      list.push(<li key={`li${idx}`}>{renderInline(li[1], `li${idx}`)}</li>);
      return;
    }
    flush(`ul${idx}`);
    const h2 = line.match(/^##\s+(.*)/);
    const h1 = line.match(/^#\s+(.*)/);
    if (h2) {
      blocks.push(
        <div key={idx} style={{ fontWeight: 700, fontSize: "0.95rem", marginTop: 8, color: "var(--jarvis-text-bright)" }}>
          {renderInline(h2[1], `h2${idx}`)}
        </div>,
      );
    } else if (h1) {
      blocks.push(
        <div key={idx} style={{ fontWeight: 800, fontSize: "1.05rem", marginTop: 6 }}>
          {renderInline(h1[1], `h1${idx}`)}
        </div>,
      );
    } else if (line.trim() === "") {
      blocks.push(<div key={idx} style={{ height: 6 }} />);
    } else {
      blocks.push(<div key={idx}>{renderInline(line, `p${idx}`)}</div>);
    }
  });
  flush("ul-final");
  return <div>{blocks}</div>;
}
