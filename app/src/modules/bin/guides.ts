/**
 * Which block of a tree the reader stands in and which the pointer is over, for the guide
 * lines.
 *
 * Copied from LTK Manager's `src/modules/workshop/shared/state/treeGuides.ts`.
 */
import { createContext, use, useCallback, useSyncExternalStore } from "react";

/** The blocks a tree's guides mark, each by the key of the row holding it. */
export interface GuideFocus {
  /** The block of the row the reader selected, focused or was revealed to. */
  readonly active: string | null;
  /** The block of the row under the pointer. */
  readonly hover: string | null;
}

/** One tree's guide focus, held outside React so a hover redraws the guides alone. */
export interface GuideStore {
  readonly get: () => GuideFocus;
  readonly set: (next: Partial<GuideFocus>) => void;
  readonly subscribe: (listener: () => void) => () => void;
}

const NO_FOCUS: GuideFocus = { active: null, hover: null };

/** A guide store with no block marked. */
export function createGuideStore(): GuideStore {
  let focus = NO_FOCUS;
  const listeners = new Set<() => void>();
  return {
    get: () => focus,
    set: (next) => {
      const merged = { ...focus, ...next };
      if (merged.active === focus.active && merged.hover === focus.hover) return;
      focus = merged;
      for (const listener of listeners) listener();
    },
    subscribe: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** The guide store of the enclosing tree. A row outside one marks no block. */
export const GuideStoreContext = createContext<GuideStore | null>(null);

const NO_STORE = () => () => {};
const NO_LEVELS = "-1,-1";

/**
 * Which of a line's guide `blocks` the reader stands in and which the pointer is over, as
 * levels. -1 where neither is one of them.
 *
 * The snapshot is a string, so a line re-renders only when its own levels change.
 */
export function useGuideLevels(blocks: readonly string[]): {
  readonly active: number;
  readonly hover: number;
} {
  const store = use(GuideStoreContext);
  const snapshot = useCallback(() => {
    if (store === null) return NO_LEVELS;
    const { active, hover } = store.get();
    const at = (key: string | null) => (key === null ? -1 : blocks.indexOf(key));
    return `${at(active)},${at(hover)}`;
  }, [blocks, store]);
  const levels = useSyncExternalStore(store?.subscribe ?? NO_STORE, snapshot);
  const [active, hover] = levels.split(",").map(Number);
  return { active: active ?? -1, hover: hover ?? -1 };
}
