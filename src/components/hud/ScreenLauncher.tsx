import { SCREENS } from "../../screens/registry";
import { useScreensStore } from "../../state/screensStore";

// Auto-generated row of launcher buttons — one per registered screen. Sits to
// the left of the settings gear (which is at right:16). Anchored at its right
// edge so added buttons grow leftward. An open screen's button stays lit.
export function ScreenLauncher() {
  const open = useScreensStore((s) => s.open);
  const toggle = useScreensStore((s) => s.toggleScreen);

  return (
    <div
      style={{
        position: "absolute",
        top: 16,
        right: 56,
        display: "flex",
        gap: 8,
        zIndex: 10,
      }}
    >
      {SCREENS.map((s) => {
        const isOpen = open.includes(s.id);
        return (
          <button
            key={s.id}
            onClick={() => toggle(s.id)}
            aria-label={s.title}
            title={s.title}
            style={{
              background: isOpen ? "rgba(47,212,255,0.18)" : "rgba(4,8,14,0.6)",
              border: "1px solid var(--jarvis-cyan-dim)",
              color: "var(--jarvis-cyan)",
              borderRadius: 4,
              width: 32,
              height: 32,
              cursor: "pointer",
              fontSize: 15,
            }}
          >
            {s.icon}
          </button>
        );
      })}
    </div>
  );
}
