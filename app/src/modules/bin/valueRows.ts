/**
 * A value family on its row: `ValueColor`, `ValueColorRgb`, `ValueFloat`, `ValueVector2` and
 * `ValueVector3` draw their constant on their collapsed row, and a colour the band of its
 * keys, read through the projected read a level at a time.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/values/utils/valueRows.ts`. Cut: the
 * probability tables and the ranges they draw, the curve read the dock and the inspector ask
 * for and the sparkline keys. A colour's curve is read, as Manager's tree reads it; every other
 * family stops at its constant.
 */
import type { BinRow, BinRows, BinValue } from "@/lib/bindings.gen";

import { nameHash } from "./binHash";
import { fieldHash, rowKey } from "./rows";

/** One node a projected read wants. */
export interface ReadRequest {
  /** The node's key, its entry and its wire path, as `rowKey` writes one. */
  readonly key: string;
  /** How many rows sit under it, which the row asking for them already carries. */
  readonly rows: number;
}

/** How a value class draws its constant. */
export type ValueFamily = "color" | "scalar" | "vector";

/**
 * The classes whose collapsed row draws its constant, by class hash.
 *
 * `ValueColorRgb` carries three channels where `ValueColor` carries four, and is read as a
 * colour with a full alpha rather than as a vector, because what it holds is a colour. An
 * `IntegratedValue*` class adds no field to the class it derives from.
 */
const FAMILY: ReadonlyMap<string, ValueFamily> = new Map([
  [nameHash("ValueColor"), "color" as const],
  [nameHash("ValueColorRgb"), "color" as const],
  [nameHash("ValueFloat"), "scalar" as const],
  [nameHash("ValueVector2"), "vector" as const],
  [nameHash("ValueVector3"), "vector" as const],
  [nameHash("IntegratedValueFloat"), "scalar" as const],
  [nameHash("IntegratedValueVector2"), "vector" as const],
  [nameHash("IntegratedValueVector3"), "vector" as const],
]);

/** The value every class of the family holds under one field hash. */
const CONSTANT = nameHash("constantValue");

/** The curve, which is a pointer and is null on a value that does not animate. */
const DYNAMICS = nameHash("dynamics");

/** The dynamics' two lists: when a stop lands, and the value there. */
const TIMES = nameHash("times");
const VALUES = nameHash("values");

/** The alpha a colour written without one carries, which is the opaque the engine samples. */
const OPAQUE = 1;

/** One key of a curve: when it lands, and its channels there. */
export interface CurveKey {
  readonly time: number;
  readonly values: readonly number[];
}

/** The family a row's value belongs to, or null for every other value. */
export function valueFamily(value: BinValue): ValueFamily | null {
  if (value.type !== "struct") return null;
  return FAMILY.get(value.classHash) ?? null;
}

/** What a value-family row draws after its class, as far as the read has answered. */
export interface ValueMark {
  readonly family: ValueFamily;
  /** The row's `constantValue`. Null until the first level answers. */
  readonly constant: BinValue | null;
  /** A colour curve's keys, in its own order. Empty until the read answers them. */
  readonly keys: readonly CurveKey[];
  /** The row's `dynamics` points at a curve, so the constant is not the whole value. */
  readonly curve: boolean;
}

/** What every level of the projected read answered, by the key each page sits under. */
export interface CurvePages {
  readonly constants: ReadonlyMap<string, BinRows>;
  readonly dynamics: ReadonlyMap<string, BinRows>;
  readonly stops: ReadonlyMap<string, BinRows>;
}

/** The first level: every family row in view, whose children are its constant and its curve. */
export function constantRequests(rows: readonly BinRow[]): ReadRequest[] {
  return rows.flatMap((row) =>
    row.value.type === "struct" && FAMILY.has(row.value.classHash)
      ? [{ key: rowKey(row), rows: row.value.len }]
      : [],
  );
}

/** The second level: the curve of every colour row whose first level answered one. */
export function dynamicsRequests(
  rows: readonly BinRow[],
  constants: ReadonlyMap<string, BinRows>,
): ReadRequest[] {
  return rows
    .filter((row) => valueFamily(row.value) === "color")
    .map((row) => readOf(under(constants.get(rowKey(row)), DYNAMICS), "struct"))
    .filter(isRead);
}

/** The third level: the two lists of every curve the second level answered for. */
export function stopRequests(dynamics: ReadonlyMap<string, BinRows>): ReadRequest[] {
  return [...dynamics.values()].flatMap((page) =>
    [TIMES, VALUES].map((field) => readOf(under(page, field), "container")).filter(isRead),
  );
}

/** What every family row in `rows` draws, out of the levels the read answered. */
export function valueMarks(
  rows: readonly BinRow[],
  pages: CurvePages,
): ReadonlyMap<string, ValueMark> {
  const marks = new Map<string, ValueMark>();
  for (const row of rows) {
    const family = valueFamily(row.value);
    if (family === null) continue;

    const key = rowKey(row);
    const page = pages.constants.get(key);
    const curve = under(page, DYNAMICS);
    const curvePage = curve === null ? undefined : pages.dynamics.get(rowKey(curve));
    marks.set(key, {
      family,
      constant: under(page, CONSTANT)?.value ?? null,
      keys: zipKeys(curvePage, pages.stops),
      curve: curve?.value.type === "struct",
    });
  }
  return marks;
}

/** The row `page` holds under `field`, or null where it holds none. */
function under(page: BinRows | undefined, field: string): BinRow | null {
  return page?.rows.find((row) => fieldHash(row.path) === field) ?? null;
}

/** The read of the rows under `row`, or null where it is not a `type` holding any. */
function readOf(row: BinRow | null, type: "container" | "struct"): ReadRequest | null {
  if (row === null) return null;

  const { value } = row;
  const held = value.type === "container" || value.type === "struct" ? value.len : 0;
  if (value.type !== type || held === 0) return null;
  return { key: rowKey(row), rows: held };
}

function isRead(read: ReadRequest | null): read is ReadRequest {
  return read !== null;
}

/** The rows of the list `page` holds under `field`, as the level below answered them. */
function listRows(
  page: BinRows | undefined,
  field: string,
  answered: ReadonlyMap<string, BinRows>,
): readonly BinRow[] {
  const row = under(page, field);
  if (row === null) return [];
  return answered.get(rowKey(row))?.rows ?? [];
}

/**
 * The keys of the curve `page` holds: its times and its values, paired by index.
 *
 * The two lists are written in step, so a key is one index of each, and a list longer than the
 * other contributes nothing past where they agree.
 */
function zipKeys(page: BinRows | undefined, answered: ReadonlyMap<string, BinRows>): CurveKey[] {
  const times = listRows(page, TIMES, answered);
  const values = listRows(page, VALUES, answered);

  const out: CurveKey[] = [];
  for (let at = 0; at < Math.min(times.length, values.length); at += 1) {
    const time = times[at]?.value;
    const held = components(values[at]?.value);
    if (time?.type !== "float" || time.value === null || held === null) continue;
    out.push({ time: time.value, values: held });
  }
  return out;
}

/** A key's value as its channels, or null for one JSON could not carry whole. */
function components(value: BinValue | undefined): number[] | null {
  if (value?.type === "float") return value.value === null ? null : [value.value];
  if (value?.type !== "vector") return null;
  const held = value.values.filter((component) => component !== null);
  return held.length === value.values.length ? held : null;
}

/** The value a mark draws as one: its lone key where the curve holds one, else its constant. */
export function markValue(mark: ValueMark): BinValue | null {
  const [key, ...rest] = mark.keys;
  if (key === undefined || rest.length > 0) return mark.constant;

  if (mark.family !== "scalar") return { type: "vector", values: [...key.values] };

  const [first] = key.values;
  return first === undefined ? mark.constant : { type: "float", value: first };
}

/**
 * The value a value-family row draws, as the one string Copy value takes.
 *
 * Null until the read lands, so the row offers no copy of a value it is not drawing.
 */
export function markText(mark: ValueMark | undefined): string | null {
  const value = mark === undefined ? null : markValue(mark);
  if (value == null) return null;
  if (mark?.family === "color") {
    const rgba = channels(value);
    return rgba === null ? null : colorHex(rgba);
  }
  if (value.type === "float") return String(value.value);
  if (value.type === "vector") return value.values.join(", ");
  return null;
}

/** The window a curve is drawn over, in the shares of a particle's life its times are. */
export interface TimeSpan {
  readonly first: number;
  readonly last: number;
}

/** The particle's own life, which a curve with no key outside it is read against. */
const LIFE: TimeSpan = { first: 0, last: 1 };

/**
 * The window `times` are drawn over: the particle's life, widened to hold every key.
 *
 * A key time is a share of that life, so 0 to 1 is the reading a modder wants and a ramp keyed
 * over the middle of it has to look like one. A file holds times outside that range, and those
 * widen the window rather than being clipped out of it.
 */
export function timeSpan(times: readonly number[]): TimeSpan {
  if (times.length === 0) return LIFE;
  return { first: Math.min(LIFE.first, ...times), last: Math.max(LIFE.last, ...times) };
}

/** Where `time` lands in `span`, as a share of it from 0 to 1. */
export function placeTime(time: number, span: TimeSpan): number {
  const width = span.last - span.first;
  return width === 0 ? 0 : (time - span.first) / width;
}

/** One stop of a colour's dynamics: when it lands, and the channels there, each 0 to 1. */
export interface ColorStop {
  readonly time: number;
  readonly rgba: readonly [number, number, number, number];
}

/** The keys of a colour as the stops its band paints, a key with no alpha being opaque. */
export function colorStops(keys: readonly CurveKey[]): ColorStop[] {
  const out: ColorStop[] = [];
  for (const key of keys) {
    const [r, g, b, a] = key.values;
    if (r === undefined || g === undefined || b === undefined) continue;
    out.push({ time: key.time, rgba: [r, g, b, a ?? OPAQUE] });
  }
  return out;
}

/**
 * A `vec4` or a `vec3` as four channels, or null for any other value.
 *
 * A component is null where the float is one JSON does not carry, and a colour missing a
 * channel is one nothing can paint. A `vec3` is a `ValueColorRgb`, which is opaque.
 */
export function channels(value: BinValue | null | undefined): ColorStop["rgba"] | null {
  if (value?.type !== "vector") return null;

  const held = components(value);
  if (held === null || (held.length !== 4 && held.length !== 3)) return null;
  return [held[0] ?? 0, held[1] ?? 0, held[2] ?? 0, held[3] ?? OPAQUE];
}

/** A colour as its bytes in hex, the channels rounded. */
export function colorHex(rgba: ColorStop["rgba"]): string {
  return `#${rgba.map(hexByte).join("")}`;
}

/** The CSS a channel set paints with, the alpha kept as a fraction. */
export function colorCss(rgba: ColorStop["rgba"]): string {
  const [r, g, b, a] = rgba;
  return `rgba(${byte(r)}, ${byte(g)}, ${byte(b)}, ${clamp(a)})`;
}

/**
 * The stops as a CSS gradient, each at its own time in the window they span.
 *
 * The outermost colours hold flat to the ends of the window, which is the value the engine
 * samples outside the keyed range. A curve whose stops share one time draws the last of them.
 */
export function gradientCss(stops: readonly ColorStop[]): string {
  const [only] = stops;
  if (only === undefined) return "";
  if (stops.length === 1) {
    const css = colorCss(only.rgba);
    return `linear-gradient(to right, ${css}, ${css})`;
  }

  const span = timeSpan(stops.map((stop) => stop.time));
  const placed = stops.map((stop) => {
    const at = placeTime(stop.time, span) * 100;
    return `${colorCss(stop.rgba)} ${at.toFixed(2)}%`;
  });
  return `linear-gradient(to right, ${placed.join(", ")})`;
}

function hexByte(channel: number): string {
  return byte(channel).toString(16).padStart(2, "0").toUpperCase();
}

function byte(channel: number): number {
  return Math.round(clamp(channel) * 255);
}

function clamp(channel: number): number {
  return Math.min(Math.max(channel, 0), 1);
}
