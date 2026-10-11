/**
 * A bin's rows as a tree's lines: the keys rows are expanded, fetched and drawn under, and the
 * visible list assembled out of the pages that answered.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/utils/binRows.ts`. Cut: the add
 * lines, the dependencies rows, a record's field hash, an entry key's hash, and the key shifts
 * an edit makes.
 */
import type { AppError, BinRow, BinRows, RowNode } from "@/lib/bindings.gen";

/** How many rows one children call answers. A longer container asks again. */
export const PAGE_SIZE = 500;

/** The key a row is expanded, fetched and drawn under: its entry and its wire path. */
export function rowKey(row: Pick<BinRow, "entry" | "path">): string {
  return `${row.entry}:${row.path}`;
}

/** The class a depth-zero row is sorted under, and null for a row that states none. */
function rootClass(row: BinRow): string | null {
  return row.value.type === "struct" ? (row.value.class ?? row.value.classHash) : null;
}

/** Named before unnamed, then by the text itself, digits by their value. */
function byText(a: string, aUnnamed: boolean, b: string, bUnnamed: boolean): number {
  if (aUnnamed !== bUnnamed) return aUnnamed ? 1 : -1;
  return a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" });
}

/**
 * A file's depth-zero rows in the order its tree lists them: objects ahead of patch targets,
 * each run by class and then by name.
 *
 * A file keeps its objects in the order its hash table fell out in, which says nothing to a
 * reader. A class or an object no name spells sorts after the named ones, by its hash.
 */
export function sortedRoots(rows: readonly BinRow[]): BinRow[] {
  return rows
    .map((row) => ({ row, class: rootClass(row) }))
    .sort((a, b) => {
      const targets = Number(a.row.node === "target") - Number(b.row.node === "target");
      if (targets !== 0) return targets;
      const classes = byText(
        a.class ?? "",
        a.row.value.type === "struct" && a.row.value.class === null,
        b.class ?? "",
        b.row.value.type === "struct" && b.row.value.class === null,
      );
      if (classes !== 0) return classes;
      return byText(a.row.name, a.row.unnamed, b.row.name, b.row.unnamed);
    })
    .map(({ row }) => row);
}

/** The key of the object row an entry hash names. */
export function objectKey(entry: string): string {
  return `${entry}:`;
}

/** The wire path of a target row, which holds the patch records of one object. */
export const TARGET_PATH = "#";

/** The key of the target row an entry hash names. */
export function targetKey(entry: string): string {
  return `${entry}:${TARGET_PATH}`;
}

/** A key's two halves: the entry hash and the wire path. */
export function splitKey(key: string): [entry: string, path: string] {
  const cut = key.indexOf(":");
  return [key.slice(0, cut), key.slice(cut + 1)];
}

/**
 * Whether `key` is `parent` or sits under it.
 *
 * A field is eight hex digits. A segment after it opens with `.`, `[` or `{`. `[3]` is not
 * under `[30]`, and `#1` is not under `#10`. Everything of an object sits under the object's
 * own key, and every record of a target under the target's.
 */
export function isUnder(parent: string, key: string): boolean {
  if (key === parent) return true;
  if (!key.startsWith(parent)) return false;
  const next = key[parent.length] ?? "";
  if (parent.endsWith(`:${TARGET_PATH}`)) return next >= "0" && next <= "9";
  if (parent.endsWith(":")) return next !== TARGET_PATH;
  return (
    key.startsWith(`${parent}.`) || key.startsWith(`${parent}[`) || key.startsWith(`${parent}{`)
  );
}

/** The hash of the field a property row's path ends in, `0x` and eight hex digits. */
export function fieldHash(path: string): string {
  return `0x${path.slice(-8)}`;
}

/** Whether rows can sit under this one. */
export function canExpand(row: BinRow): boolean {
  return childCount(row) > 0;
}

/** How many rows sit under `row`, which is what reading it costs. */
export function childCount(row: BinRow): number {
  const { value } = row;
  switch (value.type) {
    case "struct":
    case "container":
    case "map":
    case "records":
      return value.len;
    case "optional":
      return value.present ? 1 : 0;
    default:
      return 0;
  }
}

/**
 * Every key on the way down to `key`, the object's or the target's own first and `key` itself
 * last.
 *
 * A reveal opens each of them, so a row nested under a container is on screen once every level
 * has answered. A path this cannot read answers what it reached.
 */
export function ancestorKeys(key: string): string[] {
  const [entry, path] = splitKey(key);
  const record = path.startsWith(TARGET_PATH);
  const keys = [record ? targetKey(entry) : objectKey(entry)];
  let at = record ? recordEnd(path) : 0;
  if (record && at > TARGET_PATH.length) keys.push(`${entry}:${path.slice(0, at)}`);
  while (at < path.length) {
    const end = segmentEnd(path, at);
    if (end === null) break;
    at = end;
    keys.push(`${entry}:${path.slice(0, at)}`);
  }
  if (keys.at(-1) !== key) keys.push(key);
  return keys;
}

/**
 * The page a reveal waits on: the open level whose answered rows end before the next key down,
 * and how many rows it holds.
 *
 * `ancestors` are the keys down to the row, outermost first. Null while a level has not
 * answered, while its next page is on its way, and once every level holds its next key.
 */
export function revealPage(
  ancestors: readonly string[],
  childrenOf: (key: string) => LoadedChildren | undefined,
): { readonly parent: string; readonly loaded: number } | null {
  for (let at = 0; at + 1 < ancestors.length; at += 1) {
    const parent = ancestors[at]!;
    const children = childrenOf(parent);
    if (children === undefined) return null;
    const next = ancestors[at + 1];
    if (children.rows.some((row) => rowKey(row) === next)) continue;
    if (children.pending || children.error || children.rows.length >= children.total) return null;
    return { parent, loaded: children.rows.length };
  }
  return null;
}

/**
 * The row a reveal lands on: the deepest of `ancestors` drawn once every level above it has
 * answered, or null while one is still to answer.
 *
 * It is the key asked for where the tree draws it. An optional draws the leaf it holds on its
 * own row, so a place's `[0]` under one names a row the tree does not draw, and the option's
 * row stands for it; a key no answered page holds lands on the deepest row above it.
 */
export function revealLanding(
  ancestors: readonly string[],
  lineOf: (key: string) => RowLine | undefined,
  childrenOf: (key: string) => LoadedChildren | undefined,
): string | null {
  for (let at = 0; at < ancestors.length; at += 1) {
    const key = ancestors[at]!;
    const line = lineOf(key);
    if (line === undefined) return null;
    const next = ancestors[at + 1];
    if (next === undefined) return key;
    if (!line.expanded) return canExpand(line.row) ? null : key;
    const children = childrenOf(key);
    if (children === undefined) return null;
    if (children.rows.some((row) => rowKey(row) === next)) continue;
    const more = children.rows.length < children.total && children.error === undefined;
    return children.pending || more ? null : key;
  }
  return null;
}

/** Where the segment starting at `at` ends, or null for a path this cannot read. */
function segmentEnd(path: string, at: number): number | null {
  if (path[at] === "[") {
    const close = path.indexOf("]", at);
    return close < 0 ? null : close + 1;
  }
  if (path[at] === "{") {
    const end = keyEnd(path, at + 1);
    return end === null ? null : repeatEnd(path, end);
  }

  /* A field is eight hex digits, and every one but the first opens with a dot. */
  const start = path[at] === "." ? at + 1 : at;
  return start + 8 <= path.length ? start + 8 : null;
}

/** Where the record position of a record path ends: the digits after its `#`. */
function recordEnd(path: string): number {
  let end = TARGET_PATH.length;
  while (end < path.length && path[end]! >= "0" && path[end]! <= "9") end += 1;
  return end;
}

/** Where a repeated key's `#n` starting at `at` ends, or `at` where the key takes none. */
function repeatEnd(path: string, at: number): number {
  if (path[at] !== "#") return at;
  let end = at + 1;
  while (end < path.length && path[end]! >= "0" && path[end]! <= "9") end += 1;
  return end;
}

/**
 * Whether an entry row repeats the key of an earlier entry of its map.
 *
 * The backend writes the later entries of one key as `{key}#n`, so each has a path.
 */
export function repeatsKey(row: Pick<BinRow, "node" | "path">): boolean {
  return row.node === "entry" && /}#\d+$/.test(row.path);
}

/** Where the map key starting at `at` ends, quoted or bare, as the backend writes one. */
function keyEnd(path: string, at: number): number | null {
  if (path[at] !== '"') {
    const close = path.indexOf("}", at);
    return close < 0 ? null : close + 1;
  }
  let escaped = false;
  for (let scan = at + 1; scan < path.length; scan += 1) {
    if (path[scan] === "\\" && !escaped) {
      escaped = true;
      continue;
    }
    if (path[scan] === '"' && !escaped) {
      return path[scan + 1] === "}" ? scan + 2 : null;
    }
    escaped = false;
  }
  return null;
}

/** The class the rows under `row` are properties of. Null under a container, a map and a leaf. */
function ownerOf(row: BinRow): string | null {
  return row.value.type === "struct" ? row.value.classHash : null;
}

/** What the list holds for one expanded node: the pages that answered, in order. */
export interface LoadedChildren {
  readonly rows: readonly BinRow[];
  readonly total: number;
  /** A page is on its way. */
  readonly pending: boolean;
  /** A page failed. The rows end where it began. */
  readonly error?: AppError;
}

/** One line of the list: a row, or the request for a node's next page. */
export type VisibleRow =
  | {
      readonly kind: "row";
      readonly key: string;
      readonly row: BinRow;
      readonly depth: number;
      readonly expanded: boolean;
      /** Expanded, and the first page has not answered. */
      readonly loading: boolean;
      /** The class hash of the struct the row is a property of. Null for an object, an element and an entry. */
      readonly owner: string | null;
      /** The row this one is a child of. Null at depth zero. */
      readonly parent: BinRow | null;
      /** The row's position among its parent's children. */
      readonly index: number;
    }
  | {
      readonly kind: "more";
      readonly key: string;
      readonly parent: string;
      readonly depth: number;
      readonly loaded: number;
      readonly total: number;
      readonly pending: boolean;
    };

/** The line a row draws as. */
export type RowLine = Extract<VisibleRow, { kind: "row" }>;

/** The line under a node whose rows have not all answered. */
export type MoreLine = Extract<VisibleRow, { kind: "more" }>;

/**
 * The lines the list draws, in order, out of the root rows and what is fetched under the
 * expanded ones.
 *
 * The front end keeps the expansion state and the backend answers one node's children at a
 * time. A node expanded before its children answer draws as loading. A node with more rows than
 * answered draws a request for the rest under what it has.
 */
export function flattenRows(
  roots: readonly BinRow[],
  expanded: ReadonlySet<string>,
  childrenOf: (key: string) => LoadedChildren | undefined,
): VisibleRow[] {
  const out: VisibleRow[] = [];

  function visit(
    rows: readonly BinRow[],
    depth: number,
    owner: string | null,
    parent: BinRow | null,
  ) {
    rows.forEach((row, index) => {
      const key = rowKey(row);
      const isExpanded = expanded.has(key) && canExpand(row);
      const children = isExpanded ? childrenOf(key) : undefined;
      out.push({
        kind: "row",
        key,
        row,
        depth,
        expanded: isExpanded,
        loading: isExpanded && children === undefined,
        owner: row.node === "property" ? owner : null,
        parent,
        index,
      });

      if (children) {
        visit(children.rows, depth + 1, ownerOf(row), row);
        if (children.rows.length < children.total) {
          out.push({
            kind: "more",
            key: `${key}:more`,
            parent: key,
            depth: depth + 1,
            loaded: children.rows.length,
            total: children.total,
            pending: children.pending,
          });
        }
      }
    });
  }

  visit(roots, 0, null, null);
  return out;
}

/** One page of a node's children as the query answered it, or has not. */
export interface PageResult {
  readonly data?: BinRows;
  readonly error?: AppError | null;
}

/**
 * A node's pages folded into one window of rows.
 *
 * The rows end at the first page that has not answered, and the node reads as pending. A
 * failed page ends them too and carries its error. Undefined until the first page answers,
 * which is what draws a node as loading.
 */
export function mergePages(pages: readonly PageResult[]): LoadedChildren | undefined {
  const rows: BinRow[] = [];
  let total: number | null = null;
  let pending = false;
  let error: AppError | undefined;

  for (const page of pages) {
    if (page.error) {
      error = page.error;
      break;
    }
    if (!page.data) {
      pending = true;
      break;
    }
    rows.push(...page.data.rows);
    total = page.data.total;
  }

  if (total !== null) return { rows, total, pending, error };
  if (error) return { rows: [], total: 0, pending: false, error };
  return undefined;
}

/** How many pages a node wants with `loaded` rows answered and more due. */
export function pagesWanted(loaded: number): number {
  return Math.floor(loaded / PAGE_SIZE) + 1;
}

/** The key of the row a line hangs under, which names the block its innermost guide draws. */
export function lineParent(line: VisibleRow): string | null {
  if (line.depth === 0) return null;
  if (line.kind === "more") return line.parent;
  return line.parent === null ? null : rowKey(line.parent);
}

/**
 * The block each guide of a line belongs to, outermost first: the key of the row at that depth.
 *
 * `parent` is the row the line hangs under, at `depth - 1`. A guide is one block's edge, so
 * every line under that row draws the same key at the same level.
 */
export function guideBlocks(parent: string | null, depth: number): string[] {
  if (parent === null || depth === 0) return [];
  const chain = ancestorKeys(parent);
  const top = chain.length - depth;
  return Array.from({ length: depth }, (_, level) => chain[top + level] ?? "");
}

/** Whether a row of `node` draws its name outside the name column. */
export function outsideColumn(node: RowNode): boolean {
  return node === "object" || node === "target" || node === "element";
}

/** One level of depth, as the guide draws it. Characters, because the tree is mono. */
export const INDENT = "2ch";

/** Past this depth the indentation stops and the guides stack. */
export const MAX_INDENT_DEPTH = 8;

/** The narrowest the name column goes, so a shallow list is not cramped. */
const MIN_NAME_COLS = 22;

/** The widest, so one long name cannot push every value off the pane. */
const MAX_NAME_COLS = 52;

/**
 * How many characters the name column needs to hold every row without eliding.
 *
 * The tree is set in one mono face, so a character is a fixed advance and the widest row is
 * arithmetic rather than a measurement. One width for the whole list is what keeps the values
 * in a column, and taking it from the loaded rows rather than the visible ones is what stops it
 * moving while a reader scrolls.
 */
export function nameColumns(
  visible: readonly VisibleRow[],
  tagOf: (row: BinRow) => string | null,
): number {
  let widest = MIN_NAME_COLS;
  for (const line of visible) {
    /* An object, a target and an element sit outside the column, so none widens it. */
    if (line.kind !== "row" || outsideColumn(line.row.node)) continue;
    const tag = tagOf(line.row);
    const held = line.row.value.type === "struct" ? (line.row.value.class ?? "") : "";
    const cols =
      Math.min(line.depth, MAX_INDENT_DEPTH) * 2 +
      line.row.name.length +
      (tag === null ? 0 : tag.length + 1) +
      (held === "" ? 0 : held.length + 1);
    if (cols > widest) widest = cols;
  }
  return Math.min(widest, MAX_NAME_COLS);
}
