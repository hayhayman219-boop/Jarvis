import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";

// A floating, header-draggable panel. Position is remembered per `storageKey`
// (localStorage) so each AI's window stays where the user put it. Kept simple
// and dependency-free: pointer events + clamp-to-viewport.

interface Point {
  x: number;
  y: number;
}

function loadPos(key: string, fallback: Point): Point {
  try {
    const raw = localStorage.getItem(key);
    if (raw) {
      const p = JSON.parse(raw);
      if (typeof p.x === "number" && typeof p.y === "number") return p;
    }
  } catch {
    /* ignore */
  }
  return fallback;
}

export function DraggableWindow({
  storageKey,
  defaultPos,
  width,
  height,
  accent,
  title,
  headerRight,
  children,
}: {
  storageKey: string;
  defaultPos: Point;
  width: number;
  height: number;
  accent: string;
  title: ReactNode;
  headerRight?: ReactNode;
  children: ReactNode;
}) {
  const clamp = useCallback(
    (p: Point): Point => ({
      // Keep the whole window on-screen when it fits; if it's wider/taller
      // than the viewport, pin to the top-left (maxWidth/maxHeight shrink it).
      x: Math.max(0, Math.min(p.x, Math.max(0, window.innerWidth - width))),
      y: Math.max(0, Math.min(p.y, Math.max(0, window.innerHeight - height))),
    }),
    [width, height],
  );

  const [pos, setPos] = useState<Point>(() => clamp(loadPos(storageKey, defaultPos)));
  const drag = useRef<{ dx: number; dy: number } | null>(null);

  const onPointerDown = (e: React.PointerEvent) => {
    // Ignore drags that start on a button/input inside the header.
    if ((e.target as HTMLElement).closest("button,input,a,textarea")) return;
    drag.current = { dx: e.clientX - pos.x, dy: e.clientY - pos.y };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: React.PointerEvent) => {
    if (!drag.current) return;
    setPos(clamp({ x: e.clientX - drag.current.dx, y: e.clientY - drag.current.dy }));
  };
  const endDrag = () => {
    if (drag.current) {
      drag.current = null;
      try {
        localStorage.setItem(storageKey, JSON.stringify(pos));
      } catch {
        /* ignore */
      }
    }
  };

  // Keep windows on-screen if the viewport shrinks.
  useEffect(() => {
    const onResize = () => setPos((p) => clamp(p));
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [clamp]);

  return (
    <div
      style={{
        position: "absolute",
        left: pos.x,
        top: pos.y,
        width,
        height,
        maxWidth: "94vw",
        maxHeight: "calc(100vh - 24px)",
        display: "flex",
        flexDirection: "column",
        boxShadow: "0 8px 40px rgba(0,0,0,0.5)",
        borderRadius: 6,
        overflow: "hidden",
        border: `1px solid ${accent}55`,
        background: "var(--jarvis-bg)",
      }}
    >
      <div
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          gap: 8,
          padding: "6px 10px",
          cursor: "move",
          userSelect: "none",
          background: `${accent}18`,
          borderBottom: `1px solid ${accent}55`,
        }}
      >
        <span style={{ color: accent, fontWeight: 700, fontSize: "0.8rem", letterSpacing: 0.5 }}>
          {title}
        </span>
        <div style={{ display: "flex", gap: 6, alignItems: "center" }}>{headerRight}</div>
      </div>
      <div style={{ flex: 1, minHeight: 0, display: "flex" }}>{children}</div>
    </div>
  );
}
