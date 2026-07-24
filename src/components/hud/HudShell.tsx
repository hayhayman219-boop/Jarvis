import { useEffect, useRef, useState, type ReactNode } from "react";

const STAGE_WIDTH = 1920;
const STAGE_HEIGHT = 1080;

export function HudShell({ children }: { children: ReactNode }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const updateScale = (width: number, height: number) => {
      setScale(Math.min(width / STAGE_WIDTH, height / STAGE_HEIGHT));
    };
    updateScale(el.clientWidth, el.clientHeight);
    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) updateScale(entry.contentRect.width, entry.contentRect.height);
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={containerRef}
      style={{
        height: "100vh",
        width: "100vw",
        overflow: "hidden",
        position: "relative",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div className="jarvis-scanlines" />
      <div className="jarvis-grid" />
      <div
        style={{
          width: STAGE_WIDTH,
          height: STAGE_HEIGHT,
          position: "relative",
          transform: `scale(${scale})`,
          transformOrigin: "center center",
          flexShrink: 0,
        }}
      >
        {children}
      </div>
    </div>
  );
}
