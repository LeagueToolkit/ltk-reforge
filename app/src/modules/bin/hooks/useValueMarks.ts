/**
 * The constants, and a colour's keys, of the value-family rows in view.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/values/hooks/useValueMarks.ts`. Cut: the
 * three levels of the probability tables, which a curve read alone asks for.
 */
import { createContext, use, useEffect, useMemo, useRef, useState } from "react";

import type { BinRow } from "@/lib/bindings.gen";

import { type BinRef, useBinRead } from "../api";
import { rowKey } from "../rows";
import {
  constantRequests,
  dynamicsRequests,
  stopRequests,
  valueFamily,
  type ValueMark,
  valueMarks,
} from "../valueRows";

/** How long a scroll stands still before the rows it left in view are read. */
const SETTLE_MS = 180;

const NO_MARKS: ReadonlyMap<string, ValueMark> = new Map();
const NO_ROWS: readonly BinRow[] = [];

/** What the enclosing tree read for its value rows. A row outside one draws its class alone. */
export const ValueMarksContext = createContext<ReadonlyMap<string, ValueMark>>(NO_MARKS);

/** What the row under `key` draws after its class, or undefined while nothing has answered. */
export function useValueMark(key: string): ValueMark | undefined {
  return use(ValueMarksContext).get(key);
}

/**
 * The constant of every value-family row in `rows`, and a colour's keys.
 *
 * "A value family on its row" in LTK Manager's `docs/ux/BIN_EDITOR.md`. Three levels of the
 * projected read answer a colour: the row's own children, the curve one of them points at, and
 * the curve's two lists. Each level knows how many rows the next costs, so no level guesses at
 * the call's cap.
 */
export function useValueMarks(
  bin: BinRef,
  rows: readonly BinRow[],
): ReadonlyMap<string, ValueMark> {
  const settled = useSettled(rows);
  const constants = useBinRead(
    bin,
    useMemo(() => constantRequests(settled), [settled]),
  );
  const dynamics = useBinRead(
    bin,
    useMemo(() => dynamicsRequests(settled, constants), [settled, constants]),
  );
  const stops = useBinRead(
    bin,
    useMemo(() => stopRequests(dynamics), [dynamics]),
  );
  return useMemo(
    () => valueMarks(settled, { constants, dynamics, stops }),
    [settled, constants, dynamics, stops],
  );
}

/** `value` as it stood once it stopped changing for `delay` milliseconds. */
function useDebounced<T>(value: T, delay: number): T {
  const [held, setHeld] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setHeld(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return held;
}

/**
 * The value rows of `rows` as of the last moment the set stopped changing.
 *
 * A scroll changes what is in view every frame, and a call per frame is what the projected
 * read exists to avoid. Between settles the last set stands, so a row already read keeps its
 * mark and a row just scrolled in waits for the scroll to stop.
 */
function useSettled(rows: readonly BinRow[]): readonly BinRow[] {
  const family = rows.filter((row) => valueFamily(row.value) !== null);
  const signature = family.map((row) => rowKey(row)).join("\n");
  const settled = useDebounced(signature, SETTLE_MS);

  /* Held with its signature, so a settled set keeps one identity across renders. */
  const held = useRef({ signature: "", rows: NO_ROWS });
  if (settled === signature && held.current.signature !== signature) {
    held.current = { signature, rows: family };
  }
  return held.current.rows;
}
