import { useEffect, useState } from "react";

function pad(n: number) {
  return String(n).padStart(2, "0");
}

export function TopBar() {
  const [now, setNow] = useState(new Date());

  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);

  const clockTime = `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())}`;
  const clockDate = now
    .toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric", year: "numeric" })
    .toUpperCase();

  return (
    <div
      style={{
        position: "absolute",
        top: 36,
        left: 56,
        right: 56,
        display: "flex",
        justifyContent: "space-between",
        alignItems: "flex-start",
      }}
    >
      <div>
        <div
          style={{
            fontFamily: "var(--jarvis-font-display)",
            fontWeight: 700,
            fontSize: 15,
            letterSpacing: 8,
            color: "var(--jarvis-cyan)",
            opacity: 0.85,
          }}
        >
          J.A.R.V.I.S.
        </div>
        <div style={{ fontSize: 13, letterSpacing: 3, color: "var(--jarvis-text-dim)", marginTop: 6 }}>
          JUST A RATHER VERY INTELLIGENT SYSTEM &nbsp;/&nbsp; ONLINE
        </div>
      </div>
      <div style={{ textAlign: "right" }}>
        <div
          style={{
            fontFamily: "var(--jarvis-font-display)",
            fontSize: 34,
            fontWeight: 600,
            color: "var(--jarvis-text-bright)",
            letterSpacing: 2,
          }}
        >
          {clockTime}
        </div>
        <div style={{ fontSize: 13, letterSpacing: 3, color: "var(--jarvis-text-dim)", marginTop: 4 }}>
          {clockDate}
        </div>
      </div>
    </div>
  );
}
