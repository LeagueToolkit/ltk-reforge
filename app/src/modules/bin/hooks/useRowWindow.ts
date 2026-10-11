/**
 * The lines of a tree that are on screen, virtualized and measured row by row.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/hooks/useRowWindow.ts`, with
 * `measureRow` from its `src/utils/virtualRows.ts` and `instantScroll` from its
 * `src/modules/workshop/shared/utils/instantScroll.ts`. Cut: the zoom, which this app's
 * settings do not change.
 */
import {
  elementScroll,
  measureElement,
  useVirtualizer,
  type VirtualItem,
  type Virtualizer,
} from "@tanstack/react-virtual";
import { type RefObject, useCallback, useMemo } from "react";

import type { VisibleRow } from "../rows";

/**
 * One line, which is what sizes the virtualizer. A matrix opened in place grows past it.
 *
 * `min-h-6` is six spacing units of 4.5px. An estimate off the drawn height moves every row
 * below a row as it measures.
 */
export const ROW_HEIGHT = 27;

/** The room above the first row and below the last, in pixels. */
const LIST_PADDING = 4;

/** The lines of a tree that are on screen, and where each one sits. */
export interface RowWindow {
  /** The items to draw, each carrying its index and its offset. */
  readonly items: readonly VirtualItem[];
  /** The lines the items point at, in the same order. */
  readonly lines: readonly VisibleRow[];
  /** The height the whole list would take. */
  readonly totalSize: number;
  /** The ref a drawn line takes, which reports its height back. */
  readonly measureElement: (node: Element | null) => void;
  /**
   * Scroll a line to the top, or only as far as it shows with `"auto"`. False where the tree
   * contains no such line.
   */
  readonly scrollToKey: (key: string, align?: "start" | "auto") => boolean;
}

/**
 * The virtualizer's element scroll, instant unless its caller asks for smooth.
 *
 * The app's `scroll-behavior: smooth` animates a plain `scrollTo`. A reveal re-aims on every
 * frame a row measures, and each re-aim restarts that animation.
 */
function instantScroll(
  offset: number,
  { adjustments, behavior }: { adjustments?: number; behavior?: ScrollBehavior },
  instance: Virtualizer<HTMLDivElement, Element>,
): void {
  elementScroll(
    offset,
    { adjustments, behavior: behavior === "smooth" ? "smooth" : "instant" },
    instance,
  );
}

/**
 * A measured row's height, or its last known height while a hidden pane lays it out at zero.
 *
 * A tab kept mounted hides with `display: none`, where the resize observer reports every row
 * as zero tall. Caching those zeroes shrinks the list under its scroll offset, and the browser
 * clamps the offset to the shrunk height when the tab is shown again.
 */
function measureRow(
  element: Element,
  entry: ResizeObserverEntry | undefined,
  instance: Virtualizer<HTMLDivElement, Element>,
): number {
  const size = measureElement(element, entry, instance);
  if (size > 0 || element.getClientRects().length > 0) return size;

  const index = instance.indexFromElement(element);
  return instance.measurementsCache[index]?.size ?? instance.options.estimateSize(index);
}

/**
 * A window over `visible`, measured row by row.
 *
 * A row is a line of text until it holds an open matrix, so the heights are measured rather
 * than assumed and `ROW_HEIGHT` is only the estimate an unmeasured row takes.
 */
export function useRowWindow(
  scrollRef: RefObject<HTMLDivElement | null>,
  visible: readonly VisibleRow[],
): RowWindow {
  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 16,
    getItemKey: (index) => visible[index]?.key ?? index,
    /* The list's room above and below its rows is the virtualizer's, not the box's padding: a
       row scrolled to the top then sits flush with it, where padding would show a strip of
       the row above. */
    paddingStart: LIST_PADDING,
    paddingEnd: LIST_PADDING,
    scrollToFn: instantScroll,
    measureElement: measureRow,
  });

  const items = virtualizer.getVirtualItems();
  const lines = useMemo(() => items.flatMap((item) => visible[item.index] ?? []), [items, visible]);

  const scrollToKey = useCallback(
    (key: string, align: "start" | "auto" = "start") => {
      const index = visible.findIndex((line) => line.key === key);
      if (index < 0) return false;
      virtualizer.scrollToIndex(index, { align });
      return true;
    },
    [visible, virtualizer],
  );

  return {
    items,
    lines,
    totalSize: virtualizer.getTotalSize(),
    measureElement: virtualizer.measureElement,
    scrollToKey,
  };
}
