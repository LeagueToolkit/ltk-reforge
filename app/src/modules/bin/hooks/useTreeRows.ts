/**
 * The lines of one bin's tree, over the expansion state this holds.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/hooks/useTreeRows.ts`. Cut: the add
 * lines, the pinned dependencies, the link groups, the reach and the remap an edit asks for.
 */
import { useCallback, useEffect, useMemo, useState } from "react";

import type { BinRow } from "@/lib/bindings.gen";

import { type BinRef, type ChildrenRequest, useBinChildren } from "../api";
import { flattenRows, isUnder, type LoadedChildren, pagesWanted, type VisibleRow } from "../rows";

/** The lines of a tree, and the ways a reader and the window change them. */
export interface TreeRows {
  /** The lines to draw, in order. */
  readonly visible: VisibleRow[];
  /** What answered under each expanded key, which is where a row's error is. */
  readonly loaded: ReadonlyMap<string, LoadedChildren>;
  /** Open a closed row, or close an open one and forget what was open under it. */
  readonly toggle: (key: string) => void;
  /** Collapse every open row, which fetches nothing. */
  readonly collapseAll: () => void;
  /** Open every key, which is how a reveal reaches a row nested under others. */
  readonly expand: (keys: Iterable<string>) => void;
  /** Ask a node with `loadedCount` rows answered for its next page. */
  readonly requestMore: (parent: string, loadedCount: number) => void;
}

/**
 * The lines of one bin, over the expansion state this holds.
 *
 * The tree stays in the backend, so an expanded key is a fetch of the rows under it and the
 * flattening into lines happens here. `initialExpanded` are the keys open at mount.
 */
export function useTreeRows(
  bin: BinRef,
  roots: readonly BinRow[],
  initialExpanded: readonly string[],
): TreeRows {
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(() => new Set(initialExpanded));
  const [pages, setPages] = useState<ReadonlyMap<string, number>>(() => new Map());

  const requests = useMemo<ChildrenRequest[]>(
    () => [...expanded].map((key) => ({ key, pages: pages.get(key) ?? 1 })),
    [expanded, pages],
  );
  const loaded = useBinChildren(bin, requests);

  const visible = useMemo(
    () => flattenRows(roots, expanded, (key) => loaded.get(key)),
    [roots, expanded, loaded],
  );

  const toggle = useCallback((key: string) => {
    setExpanded((current) => {
      if (!current.has(key)) return new Set([...current, key]);
      /* Collapsing forgets what was open underneath. Nothing hidden is fetched. */
      return new Set([...current].filter((open) => !isUnder(key, open)));
    });
  }, []);

  const collapseAll = useCallback(() => {
    setExpanded((current) => (current.size === 0 ? current : new Set()));
  }, []);

  const expand = useCallback((keys: Iterable<string>) => {
    setExpanded((current) => {
      const next = new Set([...current, ...keys]);
      return next.size === current.size ? current : next;
    });
  }, []);

  const requestMore = useCallback((parent: string, loadedCount: number) => {
    setPages((current) => {
      const wanted = pagesWanted(loadedCount);
      if ((current.get(parent) ?? 1) >= wanted) return current;
      return new Map(current).set(parent, wanted);
    });
  }, []);

  return { visible, loaded, toggle, collapseAll, expand, requestMore };
}

/** Ask for a node's next page while the line under its rows is on screen. */
export function useNextPages(
  lines: readonly VisibleRow[],
  requestMore: (parent: string, loadedCount: number) => void,
): void {
  useEffect(() => {
    for (const line of lines) {
      if (line.kind === "more" && !line.pending) requestMore(line.parent, line.loaded);
    }
  }, [lines, requestMore]);
}
