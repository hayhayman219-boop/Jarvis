import { useEffect, useMemo, useState } from "react";
import { PopupScreen } from "../hud/PopupScreen";
import { POKEMON_GAMES } from "../../data/pokemonGames";
import { getSetting, setSetting } from "../../lib/persistedStore";

const KEY = "pokemonChecked";

// A persistent check-off list of the mainline Pokémon games, grouped by
// generation. New releases added to pokemonGames.ts appear here automatically.
export function PokemonChecklist({
  onClose,
  index,
}: {
  onClose: () => void;
  index: number;
}) {
  const [checked, setChecked] = useState<Set<string>>(new Set());

  useEffect(() => {
    getSetting<string[]>(KEY).then((a) => setChecked(new Set(a ?? [])));
  }, []);

  const toggle = (id: string) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      void setSetting(KEY, [...next]);
      return next;
    });
  };

  const byGen = useMemo(() => {
    const map = new Map<number, typeof POKEMON_GAMES>();
    for (const g of POKEMON_GAMES) {
      const arr = map.get(g.gen) ?? [];
      arr.push(g);
      map.set(g.gen, arr);
    }
    return [...map.entries()].sort((a, b) => a[0] - b[0]);
  }, []);

  const done = POKEMON_GAMES.filter((g) => checked.has(g.id)).length;
  const total = POKEMON_GAMES.length;
  const pct = Math.round((done / total) * 100);

  return (
    <PopupScreen title="Pokémon Games" onClose={onClose} index={index} width={520} height={580}>
      <div style={{ padding: 16, display: "flex", flexDirection: "column", gap: 14 }}>
        {/* Progress */}
        <div>
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 13, marginBottom: 6 }}>
            <span className="jarvis-label">Collection progress</span>
            <span style={{ color: "var(--jarvis-cyan)" }}>
              {done} / {total} · {pct}%
            </span>
          </div>
          <div style={{ height: 6, borderRadius: 3, background: "rgba(47,212,255,0.12)", overflow: "hidden" }}>
            <div style={{ width: `${pct}%`, height: "100%", background: "var(--jarvis-cyan)", transition: "width 0.2s" }} />
          </div>
        </div>

        {byGen.map(([gen, games]) => (
          <section key={gen} style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <div
              style={{
                fontSize: 11,
                letterSpacing: 3,
                textTransform: "uppercase",
                color: "var(--jarvis-cyan)",
                opacity: 0.8,
                borderBottom: "1px solid rgba(47,212,255,0.2)",
                paddingBottom: 4,
                marginBottom: 2,
              }}
            >
              Generation {gen}
            </div>
            {games.map((g) => {
              const on = checked.has(g.id);
              return (
                <label
                  key={g.id}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 10,
                    padding: "5px 6px",
                    borderRadius: 4,
                    cursor: "pointer",
                    background: on ? "rgba(47,212,255,0.08)" : "transparent",
                  }}
                >
                  <input type="checkbox" checked={on} onChange={() => toggle(g.id)} />
                  <span
                    style={{
                      flex: 1,
                      fontSize: 14,
                      textDecoration: on ? "line-through" : "none",
                      opacity: on ? 0.7 : 1,
                    }}
                  >
                    {g.title}
                  </span>
                  <span style={{ fontSize: 11, color: "var(--jarvis-text-dim)", whiteSpace: "nowrap" }}>
                    {g.year} · {g.platform}
                  </span>
                </label>
              );
            })}
          </section>
        ))}
      </div>
    </PopupScreen>
  );
}
