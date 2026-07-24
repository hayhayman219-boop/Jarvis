import { useSystemStatusStore } from "../../state/systemStatusStore";

const R = 30;
const CIRC = 2 * Math.PI * R;

function Gauge({
  label,
  pct,
  display,
  warn,
}: {
  label: string;
  pct: number;
  display: string;
  warn?: boolean;
}) {
  const color = warn ? "var(--jarvis-red)" : "var(--jarvis-cyan)";
  const clamped = Math.max(0, Math.min(100, pct));
  const dash = `${(clamped / 100) * CIRC} ${CIRC}`;
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 6, width: 76 }}>
      <div style={{ position: "relative", width: 72, height: 72 }}>
        <svg width="72" height="72" style={{ transform: "rotate(-90deg)" }}>
          <circle cx="36" cy="36" r={R} fill="none" stroke="rgba(47,212,255,0.12)" strokeWidth="5" />
          <circle
            cx="36"
            cy="36"
            r={R}
            fill="none"
            stroke={color}
            strokeWidth="5"
            strokeLinecap="round"
            strokeDasharray={dash}
            style={{ filter: `drop-shadow(0 0 3px ${color})`, transition: "stroke-dasharray 0.6s ease" }}
          />
        </svg>
        <div
          style={{
            position: "absolute",
            inset: 0,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 14,
            color: "var(--jarvis-text-bright)",
          }}
        >
          {display}
        </div>
      </div>
      <div style={{ fontSize: 10, letterSpacing: 2, color: "var(--jarvis-text-dim)" }}>{label}</div>
    </div>
  );
}

// Live system monitor: CPU, RAM, disk, temperature — bottom-right of the HUD.
export function GaugesRow() {
  const cpu = useSystemStatusStore((s) => s.cpuPercent);
  const mem = useSystemStatusStore((s) => s.memoryPercent);
  const disk = useSystemStatusStore((s) => s.diskPercent);
  const temp = useSystemStatusStore((s) => s.temperatureC);

  return (
    <div style={{ position: "absolute", bottom: 24, right: 28, display: "flex", gap: 16, zIndex: 5 }}>
      <Gauge label="CPU" pct={cpu} display={`${Math.round(cpu)}%`} warn={cpu > 90} />
      <Gauge label="RAM" pct={mem} display={`${Math.round(mem)}%`} warn={mem > 90} />
      <Gauge label="DISK" pct={disk} display={`${Math.round(disk)}%`} warn={disk > 90} />
      {temp != null && (
        <Gauge label="TEMP" pct={temp} display={`${Math.round(temp)}°`} warn={temp > 85} />
      )}
    </div>
  );
}
