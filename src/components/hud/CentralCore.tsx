import type { ReactorStatus } from "../../lib/types";

const CIRCUMFERENCE_150 = 2 * Math.PI * 150;

export function CentralCore({
  status,
  cpuPercent,
  onClick,
}: {
  status: ReactorStatus;
  cpuPercent: number;
  onClick: () => void;
}) {
  const accent = status === "listening" ? "var(--jarvis-gold)" : "var(--jarvis-cyan)";
  const coreLoad = Math.round(cpuPercent);
  const coreDashArray = `${(coreLoad / 100) * CIRCUMFERENCE_150} ${CIRCUMFERENCE_150}`;

  return (
    <button
      onClick={onClick}
      aria-label="Open Jarvis chat"
      style={{
        position: "absolute",
        top: "50%",
        left: "50%",
        transform: "translate(-50%, -50%)",
        width: 520,
        height: 520,
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "none",
        border: "none",
        padding: 0,
        cursor: "pointer",
      }}
    >
      <div
        style={{
          position: "absolute",
          width: 520,
          height: 520,
          borderRadius: "50%",
          border: "1px dashed rgba(47,212,255,0.18)",
          animation: "rotate-slow 40s linear infinite",
        }}
      />
      <div
        style={{
          position: "absolute",
          width: 440,
          height: 440,
          borderRadius: "50%",
          border: "1px solid rgba(47,212,255,0.25)",
        }}
      />

      <svg
        width="440"
        height="440"
        style={{ position: "absolute", animation: "rotate-slow 24s linear infinite" }}
      >
        <circle
          cx="220"
          cy="220"
          r="200"
          fill="none"
          stroke={accent}
          strokeOpacity="0.55"
          strokeWidth="2"
          strokeDasharray="6 14"
        />
      </svg>
      <svg
        width="380"
        height="380"
        style={{ position: "absolute", animation: "rotate-rev 30s linear infinite" }}
      >
        <circle
          cx="190"
          cy="190"
          r="170"
          fill="none"
          stroke={accent}
          strokeOpacity="0.35"
          strokeWidth="1"
          strokeDasharray="2 10"
        />
      </svg>

      <svg width="340" height="340" style={{ position: "absolute", transform: "rotate(-90deg)" }}>
        <circle cx="170" cy="170" r="150" fill="none" stroke="rgba(47,212,255,0.12)" strokeWidth="4" />
        <circle
          cx="170"
          cy="170"
          r="150"
          fill="none"
          stroke={accent}
          strokeWidth="4"
          strokeLinecap="round"
          strokeDasharray={coreDashArray}
          style={{ filter: `drop-shadow(0 0 6px ${accent})`, transition: "stroke-dasharray 0.6s ease" }}
        />
      </svg>

      <div
        style={{
          position: "absolute",
          width: 180,
          height: 180,
          borderRadius: "50%",
          border: `1px solid ${accent}`,
          animation: "pulse-ring 3s ease-out infinite",
        }}
      />
      <div
        style={{
          position: "absolute",
          width: 180,
          height: 180,
          borderRadius: "50%",
          border: `1px solid ${accent}`,
          animation: "pulse-ring 3s ease-out infinite 1.5s",
        }}
      />

      <div
        style={{
          width: 170,
          height: 170,
          borderRadius: "50%",
          background: `radial-gradient(circle at 40% 35%, #eafcff 0%, ${accent} 30%, #06333d 75%, transparent 100%)`,
          boxShadow: `0 0 60px 10px ${accent === "var(--jarvis-gold)" ? "rgba(232,176,75,0.55)" : "var(--jarvis-cyan-glow)"}, inset 0 0 30px rgba(255,255,255,0.3)`,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          animation: "flicker 5s ease-in-out infinite",
        }}
      >
        <div style={{ textAlign: "center" }}>
          <div style={{ fontFamily: "var(--jarvis-font-display)", fontSize: 38, fontWeight: 900, color: "#02181c" }}>
            {coreLoad}%
          </div>
          <div style={{ fontSize: 11, letterSpacing: 3, color: "#02181c", opacity: 0.7, marginTop: 2 }}>
            SYSTEM LOAD
          </div>
        </div>
      </div>
    </button>
  );
}
