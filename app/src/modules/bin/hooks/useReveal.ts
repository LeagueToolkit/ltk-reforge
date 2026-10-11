/**
 * A row the tree is asked to open down to, focus and scroll to the top.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/hooks/useReveal.ts`. Added: the
 * landing, so a key the tree draws no row for lands on the deepest row above it.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { BinRow } from "@/lib/bindings.gen";

import {
  ancestorKeys,
  isUnder,
  type LoadedChildren,
  revealLanding,
  revealPage,
  rowKey,
  type RowLine,
  type VisibleRow,
} from "../rows";

/** A row the tree is asked to expand, focus and scroll to. A new token scrolls again. */
export interface TreeReveal {
  readonly key: string;
  readonly token: number;
}

/** The row a reveal landed on, until a reader moves. */
export interface Revealed {
  readonly focused: string | null;
  readonly clearFocus: () => void;
}

/** What a reveal reads and changes of the tree it opens. */
export interface RevealTree {
  readonly roots: readonly BinRow[];
  readonly visible: readonly VisibleRow[];
  readonly loaded: ReadonlyMap<string, LoadedChildren>;
  readonly expand: (keys: Iterable<string>) => void;
  /** Ask a node with `loadedCount` rows answered for its next page. */
  readonly requestMore: (parent: string, loadedCount: number) => void;
  readonly scrollToKey: (key: string) => boolean;
}

/**
 * The row a reveal opens down to, focuses and scrolls to.
 *
 * Every level above the row opens, and a level whose answered pages end before the next key
 * down asks for its next page, so a row past a page boundary is reached too. The scroll waits
 * for every level to answer. A request for a row this tree does not hold is left alone.
 */
export function useReveal(
  reveal: TreeReveal | null,
  { roots, visible, loaded, expand, requestMore, scrollToKey }: RevealTree,
): Revealed {
  const [focused, setFocused] = useState<string | null>(null);
  const [scrollTo, setScrollTo] = useState<TreeReveal | null>(null);

  const ancestors = useMemo(() => {
    if (reveal === null) return [];
    return ancestorKeys(reveal.key).filter((key) => roots.some((row) => isUnder(rowKey(row), key)));
  }, [reveal, roots]);

  useEffect(() => {
    if (reveal === null || ancestors.length === 0) return;
    expand(ancestors);
    setScrollTo(reveal);
  }, [reveal, ancestors, expand]);

  const childrenOf = useCallback((key: string) => loaded.get(key), [loaded]);
  const page = scrollTo === null ? null : revealPage(ancestors, childrenOf);
  const pageParent = page?.parent ?? null;
  const pageLoaded = page?.loaded ?? 0;
  useEffect(() => {
    if (pageParent !== null) requestMore(pageParent, pageLoaded);
  }, [pageParent, pageLoaded, requestMore]);

  const lines = useMemo(() => {
    const rows = new Map<string, RowLine>();
    for (const line of visible) if (line.kind === "row") rows.set(line.key, line);
    return rows;
  }, [visible]);
  const landing =
    scrollTo === null ? null : revealLanding(ancestors, (key) => lines.get(key), childrenOf);

  /* Keyed on the request itself, so a second request for the same row scrolls again. */
  const scrolled = useRef<TreeReveal | null>(null);
  useEffect(() => {
    if (scrollTo === null || landing === null || scrolled.current === scrollTo) return;
    if (scrollToKey(landing)) {
      scrolled.current = scrollTo;
      setFocused(landing);
    }
  }, [scrollTo, landing, scrollToKey]);

  const clearFocus = useCallback(() => setFocused(null), []);

  return { focused, clearFocus };
}
