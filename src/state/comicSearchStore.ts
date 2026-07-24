import { create } from "zustand";
import { comicVineSearch, type ComicResult } from "../lib/apiClient";

// Holds the latest Comic Vine lookup so the ComicCovers pop-up screen can
// render it. The chat intent (or the screen's own search box) drives `search`.
interface ComicSearchState {
  query: string;
  results: ComicResult[];
  loading: boolean;
  error: string | null;
  search: (query: string) => Promise<void>;
}

export const useComicSearchStore = create<ComicSearchState>((set) => ({
  query: "",
  results: [],
  loading: false,
  error: null,
  search: async (query) => {
    const q = query.trim();
    if (!q) return;
    set({ query: q, loading: true, error: null });
    try {
      const results = await comicVineSearch(q);
      set({ results, loading: false, error: results.length ? null : "No comics found." });
    } catch (e) {
      set({ results: [], loading: false, error: String(e) });
    }
  },
}));
