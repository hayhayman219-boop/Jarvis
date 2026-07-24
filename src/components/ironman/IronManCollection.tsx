import { useEffect, useMemo, useState } from "react";
import { PopupScreen } from "../hud/PopupScreen";
import { IRON_MAN_COLLECTION } from "../../data/ironManCollection";
import { getSetting, setSetting } from "../../lib/persistedStore";

const OWNED_KEY = "ironManOwned";
const WANT_KEY = "ironManWanted";

// Track which key Iron Man comics/films you Own (✓) or Want (★). Both persist.
export function IronManCollection({ onClose, index }: { onClose: () => void; index: number }) {
  const [owned, setOwned] = useState<Set<string>>(new Set());
  const [wanted, setWanted] = useState<Set<string>>(new Set());

  useEffect(() => {
    getSetting<string[]>(OWNED_KEY).then((a) => setOwned(new Set(a ?? [])));
    getSetting<string[]>(WANT_KEY).then((a) => setWanted(new Set(a ?? [])));
  }, []);

  const toggle = (
    id: string,
    set: React.Dispatch<React.SetStateAction<Set<string>>>,
    key: string,
  ) => {
    set((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      void setSetting(key, [...next]);
      return next;
    });
  };

  const byCategory = useMemo(() => {
    const map = new Map<string, typeof IRON_MAN_COLLECTION>();
    for (const it of IRON_MAN_COLLECTION) {
      const arr = map.get(it.category) ?? [];
      arr.push(it);
      map.set(it.category, arr);
    }
    return [...map.entries()];
  }, []);

  const ownedCount = owned.size;
  const total = IRON_MAN_COLLECTION.length;
  const pct = Math.round((ownedCount / total) * 100);

  return (
    <PopupScreen title="Iron Man Collection" onClose={onClose} index={index} width={560} height={600}>
      <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 14 }}>
        <div>
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 13, marginBottom: 6 }}>
            <span className="jarvis-label">Owned</span>
            <span style={{ color: "var(--jarvis-gold)" }}>
              {ownedCount} / {total} · {pct}% · {wanted.size} on wishlist
            </span>
          </div>
          <div style={{ height: 6, borderRadius: 3, background: "rgba(255,184,77,0.12)", overflow: "hidden" }}>
            <div style={{ width: `${pct}%`, height: "100%", background: "var(--jarvis-gold)", transition: "width 0.2s" }} />
          </div>
        </div>

        {byCategory.map(([cat, items]) => (
          <section key={cat} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <div
              style={{
                fontSize: 11,
                letterSpacing: 3,
                textTransform: "uppercase",
                color: "var(--jarvis-gold)",
                opacity: 0.85,
                borderBottom: "1px solid rgba(255,184,77,0.2)",
                paddingBottom: 4,
                marginBottom: 2,
              }}
            >
              {cat}
            </div>
            {items.map((it) => {
              const own = owned.has(it.id);
              const want = wanted.has(it.id);
              return (
                <div
                  key={it.id}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 10,
                    padding: "5px 6px",
                    borderRadius: 4,
                    background: own ? "rgba(255,184,77,0.08)" : "transparent",
                  }}
                >
                  <button
                    onClick={() => toggle(it.id, setOwned, OWNED_KEY)}
                    title={own ? "Owned — click to unset" : "Mark owned"}
                    style={{
                      width: 24,
                      height: 24,
                      borderRadius: 4,
                      cursor: "pointer",
                      border: `1px solid ${own ? "var(--jarvis-gold)" : "var(--jarvis-cyan-dim)"}`,
                      background: own ? "var(--jarvis-gold)" : "transparent",
                      color: own ? "var(--jarvis-bg)" : "var(--jarvis-text-dim)",
                      fontSize: 13,
                    }}
                  >
                    {own ? "✓" : ""}
                  </button>
                  <button
                    onClick={() => toggle(it.id, setWanted, WANT_KEY)}
                    title={want ? "On wishlist — click to remove" : "Add to wishlist"}
                    style={{
                      width: 24,
                      height: 24,
                      borderRadius: 4,
                      cursor: "pointer",
                      border: "1px solid var(--jarvis-cyan-dim)",
                      background: "transparent",
                      color: want ? "var(--jarvis-gold)" : "var(--jarvis-text-dim)",
                      fontSize: 13,
                    }}
                  >
                    {want ? "★" : "☆"}
                  </button>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 14, opacity: own ? 0.75 : 1 }}>{it.title}</div>
                    <div style={{ fontSize: 11, color: "var(--jarvis-text-dim)" }}>
                      {it.year} · {it.note}
                    </div>
                  </div>
                </div>
              );
            })}
          </section>
        ))}
      </div>
    </PopupScreen>
  );
}
