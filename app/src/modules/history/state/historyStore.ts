import { create } from "zustand";

/** A path asked about, at a build. */
export interface Asked {
  query: string;
  at: number;
}

/** An object asked for: the bin and the entry as typed or picked. */
export interface ObjectAsked {
  bin: string;
  entry: string;
}

interface HistoryValues {
  /** The head of the index the state is of. Null before the page first shows. */
  head: number | null;
  /** The search box's text. */
  text: string;
  /** The build picked, by its position. */
  at: number | null;
  /** The path looked up, whose answer is the query's. */
  asked: Asked | null;
  /** The lookup whose answer filled the object's bin, so that coming back fills it no more. */
  seeded: Asked | null;
  /** The object card's bin and entry, as typed or picked. */
  bin: string;
  entry: string;
  /** The object shown. */
  object: ObjectAsked | null;
  /** The build the object is compared at, by its position. */
  other: number | null;
}

interface HistoryState extends HistoryValues {
  /** Starts over at an index's head, unless the state is of that head already. */
  start: (head: number) => void;
  setText: (text: string) => void;
  setAt: (at: number) => void;
  /** Looks a path up at a build: the object and the comparison start over. */
  lookUp: (query: string, at: number) => void;
  /**
   * Fills the object's bin from a lookup's answer, once per lookup: the path when it is a bin,
   * else nothing.
   */
  seed: (asked: Asked, path: string, isBin: boolean) => void;
  setBin: (bin: string) => void;
  setEntry: (entry: string) => void;
  /** Shows an object picked from the answer. */
  openObject: (bin: string, entry: string) => void;
  /** Shows the object the bin and the entry name, as typed. */
  showObject: () => void;
  setOther: (other: number | null) => void;
}

const EMPTY: HistoryValues = {
  head: null,
  text: "",
  at: null,
  asked: null,
  seeded: null,
  bin: "",
  entry: "",
  object: null,
  other: null,
};

/** The History page's state, kept while the user moves between pages, as the run page's is. */
export const useHistoryStore = create<HistoryState>()((set, get) => ({
  ...EMPTY,
  start: (head) => {
    if (get().head !== head) {
      set({ ...EMPTY, head, at: head });
    }
  },
  setText: (text) => set({ text }),
  setAt: (at) => set({ at }),
  lookUp: (query, at) =>
    set({ text: query, at, asked: { query, at }, bin: "", entry: "", object: null, other: null }),
  seed: (asked, path, isBin) => {
    if (get().seeded !== asked) {
      set({ seeded: asked, bin: isBin ? path : "", entry: "", object: null });
    }
  },
  setBin: (bin) => set({ bin }),
  setEntry: (entry) => set({ entry }),
  openObject: (bin, entry) => set({ bin, entry, object: { bin, entry } }),
  showObject: () => {
    const { bin, entry } = get();
    set({ object: { bin: bin.trim(), entry: entry.trim() } });
  },
  setOther: (other) => set({ other }),
}));
