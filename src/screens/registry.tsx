import type { ReactNode } from "react";
import { NotionViewer } from "../components/notion/NotionViewer";
import { WebcamViewer } from "../components/webcam/WebcamViewer";
import { PokemonChecklist } from "../components/pokemon/PokemonChecklist";
import { IronManCollection } from "../components/ironman/IronManCollection";
import { ComicCovers } from "../components/ironman/ComicCovers";

// A registered pop-up screen. To add a new screen (webcam, checklists, …),
// append one entry here — the launcher button, the open/close voice intent,
// and the mount/unmount are all driven off this list, so nothing else needs
// touching.
export interface ScreenDef {
  id: string;
  /** Shown in the launcher tooltip and (for PopupScreen-based screens) the title bar. */
  title: string;
  /** Single glyph for the launcher button. */
  icon: string;
  /** Lowercase phrases that "open/close X" voice or chat commands match on. */
  aliases: string[];
  /** Renders the screen body. `index` is the stack position (for cascading). */
  render: (props: { onClose: () => void; index: number }) => ReactNode;
}

export const SCREENS: ScreenDef[] = [
  {
    id: "notion",
    title: "Notion",
    icon: "▤",
    aliases: ["notion", "my notes"],
    render: ({ onClose }) => <NotionViewer onClose={onClose} />,
  },
  {
    id: "webcam",
    title: "Webcam",
    icon: "◉",
    aliases: ["webcam", "camera", "the camera", "see me", "look at me"],
    render: ({ onClose, index }) => <WebcamViewer onClose={onClose} index={index} />,
  },
  {
    id: "pokemon",
    title: "Pokémon Games",
    icon: "◓",
    aliases: ["pokemon", "pokémon", "pokedex", "pokemon games", "pokemon checklist"],
    render: ({ onClose, index }) => <PokemonChecklist onClose={onClose} index={index} />,
  },
  {
    id: "ironman-collection",
    title: "Iron Man Collection",
    icon: "⬡",
    aliases: ["iron man collection", "my collection", "comic collection", "collection tracker", "my comics"],
    render: ({ onClose, index }) => <IronManCollection onClose={onClose} index={index} />,
  },
  {
    id: "comic-covers",
    title: "Comic Covers",
    icon: "▦",
    aliases: ["comic covers", "comic lookup", "comic search"],
    render: ({ onClose, index }) => <ComicCovers onClose={onClose} index={index} />,
  },
];

/** Finds the first screen whose alias appears in the given text, if any. */
export function findScreenByPhrase(text: string): ScreenDef | undefined {
  const lc = text.toLowerCase();
  return SCREENS.find((s) => s.aliases.some((a) => lc.includes(a)));
}
