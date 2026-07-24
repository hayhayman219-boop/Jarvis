import { create } from "zustand";

// Tracks which pop-up screens are currently open. Order in `open` is stack
// order (last = frontmost / most-recently opened), used to cascade and layer
// multiple screens. Screens are referenced by their registry id.
interface ScreensState {
  open: string[];
  openScreen: (id: string) => void;
  closeScreen: (id: string) => void;
  toggleScreen: (id: string) => void;
  closeAll: () => void;
}

export const useScreensStore = create<ScreensState>((set) => ({
  open: [],
  openScreen: (id) =>
    set((s) => (s.open.includes(id) ? s : { open: [...s.open, id] })),
  closeScreen: (id) => set((s) => ({ open: s.open.filter((x) => x !== id) })),
  toggleScreen: (id) =>
    set((s) =>
      s.open.includes(id)
        ? { open: s.open.filter((x) => x !== id) }
        : { open: [...s.open, id] },
    ),
  closeAll: () => set({ open: [] }),
}));
