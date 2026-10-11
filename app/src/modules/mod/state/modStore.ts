import { create } from "zustand";

import type { ExploreView } from "@/lib/bindings.gen";

/** A mod explored, with the archive as it was picked. */
export interface Explored {
  archive: string;
  view: ExploreView;
}

interface ModState {
  /** The mod archive picked. */
  archive: string | null;
  /** The last mod explored, while it is the one picked. The backend holds the same one. */
  explored: Explored | null;
  setArchive: (archive: string | null) => void;
  setExplored: (explored: Explored) => void;
}

/** The Mod page's state, kept while the user moves between pages. */
export const useModStore = create<ModState>()((set) => ({
  archive: null,
  explored: null,
  setArchive: (archive) => set({ archive, explored: null }),
  setExplored: (explored) => set({ explored }),
}));
