/**
 * The keys of a tree's rows: the WAI-ARIA tree pattern over a virtualized list.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/hooks/useTreeNavigation.ts`, with
 * `isCollapseAllKey` from its `src/modules/workshop/shared/utils/treeGestures.ts`. Cut: `Enter`
 * and `F2` opening a value for an edit.
 */
import {
  type KeyboardEvent as ReactKeyboardEvent,
  type RefObject,
  useEffect,
  useMemo,
  useState,
} from "react";

import { lineParent, type VisibleRow } from "../rows";

interface TreeNavigationOptions {
  visible: readonly VisibleRow[];
  scrollRef: RefObject<HTMLDivElement | null>;
  scrollToKey: (key: string, align?: "start" | "auto") => boolean;
  /** Changes whenever the drawn window does, so a row scrolled into it can take focus. */
  drawn: unknown;
  toggle: (key: string) => void;
}

/** The tab stop of a tree and its answer to a key. */
export interface TreeNavigation {
  /** The one row a `Tab` into the tree lands on. */
  readonly tabStop: string | null;
  /** Answer a key on a focused row, answering whether the tree took it. */
  readonly keyDown: (event: ReactKeyboardEvent<HTMLElement>) => boolean;
  /** A row took focus, by pointer or by key. */
  readonly focused: (target: EventTarget) => void;
}

/** Whether a key press on a focused tree asks it to shut every folder: `Ctrl+←`, as VS Code. */
export function isCollapseAllKey(event: {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}): boolean {
  return (
    event.key === "ArrowLeft" &&
    (event.ctrlKey || event.metaKey) &&
    !event.altKey &&
    !event.shiftKey
  );
}

/** The row element under `target`, where `target` is a row itself. */
function rowKeyOf(target: EventTarget): string | null {
  if (!(target instanceof HTMLElement)) return null;
  return target.dataset.treeRow ?? null;
}

/**
 * The arrow keys of a tree's rows.
 *
 * `Up` and `Down` walk the rows, `Right` opens a row or steps into it, `Left` shuts it or steps
 * out to its parent, `Home` and `End` reach the ends, and `Enter` opens or shuts a row. One row
 * is the tab stop.
 */
export function useTreeNavigation({
  visible,
  scrollRef,
  scrollToKey,
  drawn,
  toggle,
}: TreeNavigationOptions): TreeNavigation {
  const [current, setCurrent] = useState<string | null>(null);
  const [pending, setPending] = useState<string | null>(null);

  const rows = useMemo(
    () => visible.filter((line) => line.kind === "row").map((line) => line.key),
    [visible],
  );
  const tabStop = current !== null && rows.includes(current) ? current : (rows[0] ?? null);

  /* A row scrolled to draws a frame later, so focus waits for it to mount. */
  useEffect(() => {
    if (pending === null) return;
    const row = scrollRef.current?.querySelector<HTMLElement>(
      `[data-tree-row="${CSS.escape(pending)}"]`,
    );
    if (row === null || row === undefined) return;

    row.focus({ preventScroll: true });
    setPending(null);
  }, [drawn, pending, scrollRef]);

  function moveTo(key: string | undefined) {
    if (key === undefined) return;
    setCurrent(key);
    setPending(key);
    scrollToKey(key, "auto");
  }

  function keyDown(event: ReactKeyboardEvent<HTMLElement>): boolean {
    const key = rowKeyOf(event.target);
    if (key === null || event.altKey || event.ctrlKey || event.metaKey) return false;

    const row = event.target as HTMLElement;
    const at = rows.indexOf(key);
    const expanded = row.getAttribute("aria-expanded");

    switch (event.key) {
      case "ArrowDown":
        moveTo(rows[at + 1]);
        return true;
      case "ArrowUp":
        moveTo(rows[at - 1]);
        return true;
      case "Home":
        moveTo(rows[0]);
        return true;
      case "End":
        moveTo(rows.at(-1));
        return true;
      case "ArrowRight":
        if (expanded === "false") toggle(key);
        if (expanded === "true") moveTo(rows[at + 1]);
        return true;
      case "ArrowLeft": {
        if (expanded === "true") {
          toggle(key);
          return true;
        }
        const line = visible.find((each) => each.key === key);
        const parent = line === undefined ? null : lineParent(line);
        if (parent !== null && rows.includes(parent)) moveTo(parent);
        return true;
      }
      case "Enter":
        if (expanded === null) return false;
        toggle(key);
        return true;
      default:
        return false;
    }
  }

  function focused(target: EventTarget) {
    const key = rowKeyOf(target);
    if (key !== null) setCurrent(key);
  }

  return { tabStop, keyDown, focused };
}
