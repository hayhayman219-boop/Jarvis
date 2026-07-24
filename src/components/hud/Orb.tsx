import type { ReactorStatus } from "../../lib/types";

// A glowing hexagon core inside a rotating radar ring with orbiting nodes,
// matched to the dhaibuilds reference (electric-blue field, cyan glow). It
// reacts to state: idle breathes slowly; listening spins the radar faster
// and brightens; thinking counter-rotates the inner ring; speaking pulses
// the core in time with a faster beat.
const ACCENT = "#4db8ff";
const ACCENT_BRIGHT = "#bfe9ff";

function speedFor(status: ReactorStatus) {
  switch (status) {
    case "listening":
      return { radar: 8, inner: 14, corePulse: 2.2, glow: 1.35 };
    case "thinking":
      return { radar: 14, inner: 6, corePulse: 1.4, glow: 1.15 };
    case "speaking":
      return { radar: 12, inner: 18, corePulse: 0.9, glow: 1.5 };
    default:
      return { radar: 26, inner: 40, corePulse: 4.5, glow: 1.0 };
  }
}

export function Orb({ status = "idle" as ReactorStatus }: { status?: ReactorStatus }) {
  const s = speedFor(status);
  const cx = 250;
  const cy = 250;

  // Radar tick ring: short radial lines all the way around.
  const ticks = Array.from({ length: 72 }, (_, i) => {
    const a = (i / 72) * Math.PI * 2;
    const long = i % 6 === 0;
    const r1 = long ? 214 : 222;
    const r2 = 230;
    return (
      <line
        key={i}
        x1={cx + Math.cos(a) * r1}
        y1={cy + Math.sin(a) * r1}
        x2={cx + Math.cos(a) * r2}
        y2={cy + Math.sin(a) * r2}
        stroke={ACCENT}
        strokeWidth={long ? 2 : 1}
        strokeOpacity={long ? 0.9 : 0.4}
      />
    );
  });

  // Orbiting node dots on the mid ring.
  const nodes = [0, 55, 120, 175, 235, 300].map((deg, i) => {
    const a = (deg / 360) * Math.PI * 2;
    const r = 150;
    return (
      <circle
        key={i}
        cx={cx + Math.cos(a) * r}
        cy={cy + Math.sin(a) * r}
        r={i % 2 === 0 ? 5 : 3}
        fill={ACCENT_BRIGHT}
        style={{ filter: `drop-shadow(0 0 5px ${ACCENT})` }}
      />
    );
  });

  const hex = (r: number) =>
    Array.from({ length: 6 }, (_, i) => {
      const a = (i / 6) * Math.PI * 2 - Math.PI / 2;
      return `${cx + Math.cos(a) * r},${cy + Math.sin(a) * r}`;
    }).join(" ");

  return (
    <div className="orb-wrap" style={{ filter: `brightness(${s.glow})` }}>
      <svg viewBox="0 0 500 500" width="100%" height="100%" style={{ maxHeight: "88vh" }}>
        <defs>
          <radialGradient id="coreGlow" cx="50%" cy="45%">
            <stop offset="0%" stopColor={ACCENT_BRIGHT} stopOpacity="0.9" />
            <stop offset="45%" stopColor={ACCENT} stopOpacity="0.35" />
            <stop offset="100%" stopColor={ACCENT} stopOpacity="0" />
          </radialGradient>
        </defs>

        {/* outer radar tick ring — rotates */}
        <g style={{ transformOrigin: "250px 250px", animation: `orb-spin ${s.radar}s linear infinite` }}>
          {ticks}
        </g>

        {/* broken outer arc segments */}
        <g
          fill="none"
          stroke={ACCENT}
          strokeWidth="3"
          style={{
            transformOrigin: "250px 250px",
            animation: `orb-spin-rev ${s.radar * 1.6}s linear infinite`,
            filter: `drop-shadow(0 0 4px ${ACCENT})`,
          }}
        >
          <path d="M 250 58 A 192 192 0 0 1 400 130" strokeLinecap="round" />
          <path d="M 442 250 A 192 192 0 0 1 370 400" strokeLinecap="round" />
          <path d="M 250 442 A 192 192 0 0 1 100 370" strokeLinecap="round" />
          <path d="M 58 250 A 192 192 0 0 1 130 100" strokeLinecap="round" />
        </g>

        {/* mid dashed ring with orbiting nodes — rotates opposite */}
        <g style={{ transformOrigin: "250px 250px", animation: `orb-spin-rev ${s.inner}s linear infinite` }}>
          <circle cx={cx} cy={cy} r="150" fill="none" stroke={ACCENT} strokeWidth="1" strokeOpacity="0.35" strokeDasharray="2 8" />
          {nodes}
        </g>

        {/* corner brackets */}
        <g stroke={ACCENT_BRIGHT} strokeWidth="2" fill="none" strokeOpacity="0.8">
          <path d="M 140 118 h -18 v 18" />
          <path d="M 360 118 h 18 v 18" />
          <path d="M 140 382 h -18 v -18" />
          <path d="M 360 382 h 18 v -18" />
        </g>

        {/* core glow halo — pulses */}
        <circle
          cx={cx}
          cy={cy}
          r="95"
          fill="url(#coreGlow)"
          style={{ transformOrigin: "250px 250px", animation: `orb-pulse ${s.corePulse}s ease-in-out infinite` }}
        />

        {/* hexagon core — double stroke, strong glow */}
        <g style={{ transformOrigin: "250px 250px", animation: `orb-pulse ${s.corePulse}s ease-in-out infinite`, filter: `drop-shadow(0 0 10px ${ACCENT}) drop-shadow(0 0 22px ${ACCENT})` }}>
          <polygon points={hex(72)} fill="none" stroke={ACCENT} strokeWidth="2" strokeOpacity="0.5" />
          <polygon points={hex(60)} fill="none" stroke={ACCENT_BRIGHT} strokeWidth="3" strokeLinejoin="round" />
        </g>
      </svg>
    </div>
  );
}
