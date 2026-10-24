import { create } from "zustand";

interface TickState {
  /** The archives ticked, by path. */
  ticked: Set<string>;
  /** When the library whose default ticks these started from was read, as the query stamps it. */
  readAt: number;
  setTicked: (ticked: Set<string>, readAt?: number) => void;
}

/**
 * The ticks of the LTK Manager page, kept while the person moves between pages: a scan's
 * rows are kept too, and ticks lost on the way back would be the selection the rows asked for.
 */
export const useTickStore = create<TickState>()((set) => ({
  ticked: new Set(),
  readAt: 0,
  setTicked: (ticked, readAt) => set((state) => ({ ticked, readAt: readAt ?? state.readAt })),
}));
