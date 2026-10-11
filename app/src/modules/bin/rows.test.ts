/**
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/utils/__tests__/binRows.test.ts`.
 * Cut: an entry key's hash, a record's field, the add lines. Added: the landing of a reveal.
 */
import { describe, expect, it } from "vitest";

import type { AppError, BinRow } from "@/lib/bindings.gen";

import {
  ancestorKeys,
  canExpand,
  childCount,
  fieldHash,
  flattenRows,
  guideBlocks,
  isUnder,
  lineParent,
  type LoadedChildren,
  mergePages,
  nameColumns,
  PAGE_SIZE,
  pagesWanted,
  repeatsKey,
  revealLanding,
  revealPage,
  rowKey,
  type RowLine,
} from "./rows";

const ENTRY = "0x2a1f3c7d";

function row(overrides: Partial<BinRow>): BinRow {
  return {
    entry: ENTRY,
    path: "",
    label: "",
    node: "property",
    name: "name",
    unnamed: false,
    kind: "string",
    value: { type: "string", value: "text" },
    ...overrides,
  };
}

const OBJECT = row({
  node: "object",
  name: "Characters/Aatrox",
  kind: null,
  value: { type: "struct", classHash: "0x9b67e9f6", class: "CharacterRecord", len: 2 },
});
const SIZE = row({ path: "0000000a", label: "size", name: "size", kind: "u8" });
const ITEMS = row({
  path: "0000000b",
  label: "items",
  name: "items",
  kind: "list",
  value: { type: "container", len: 3, itemKind: "embed" },
});
const ITEM = (index: number) =>
  row({
    path: `0000000b[${index}]`,
    label: `items[${index}]`,
    name: `[${index}]`,
    node: "element",
    kind: "embed",
    value: { type: "struct", classHash: "0x0000beef", class: "Part", len: 1 },
  });
const PART_NAME = row({
  path: "0000000b[1].0000000c",
  label: "items[1].name",
  name: "name",
});

function loaded(rows: BinRow[], total = rows.length, pending = false): LoadedChildren {
  return { rows, total, pending };
}

function keysOf(visible: ReturnType<typeof flattenRows>): string[] {
  return visible.map((line) => line.key);
}

describe("flattenRows", () => {
  it("lists a collapsed root as one line whatever it holds", () => {
    const visible = flattenRows([OBJECT], new Set(), () => undefined);

    expect(keysOf(visible)).toEqual([`${ENTRY}:`]);
    expect(visible[0]).toMatchObject({ kind: "row", depth: 0, expanded: false, loading: false });
  });

  it("draws an expanded node as loading until its first page answers", () => {
    const visible = flattenRows([OBJECT], new Set([rowKey(OBJECT)]), () => undefined);

    expect(visible).toHaveLength(1);
    expect(visible[0]).toMatchObject({ kind: "row", expanded: true, loading: true });
  });

  it("nests the fetched children under their parent, one depth down", () => {
    const children = new Map([[rowKey(OBJECT), loaded([SIZE, ITEMS])]]);
    const visible = flattenRows([OBJECT], new Set([rowKey(OBJECT)]), (key) => children.get(key));

    expect(keysOf(visible)).toEqual([`${ENTRY}:`, `${ENTRY}:0000000a`, `${ENTRY}:0000000b`]);
    expect(visible.map((line) => line.depth)).toEqual([0, 1, 1]);
  });

  it("asks for the rest of a node whose pages have not covered its total", () => {
    const expanded = new Set([rowKey(OBJECT), rowKey(ITEMS)]);
    const children = new Map([
      [rowKey(OBJECT), loaded([ITEMS])],
      [rowKey(ITEMS), loaded([ITEM(0), ITEM(1)], 3, true)],
    ]);
    const visible = flattenRows([OBJECT], expanded, (key) => children.get(key));

    expect(visible.at(-1)).toEqual({
      kind: "more",
      key: `${ENTRY}:0000000b:more`,
      parent: `${ENTRY}:0000000b`,
      depth: 2,
      loaded: 2,
      total: 3,
      pending: true,
    });
  });

  it("names the class a property is read on, and none for an element", () => {
    const expanded = new Set([rowKey(OBJECT), rowKey(ITEMS), rowKey(ITEM(1))]);
    const children = new Map([
      [rowKey(OBJECT), loaded([SIZE, ITEMS])],
      [rowKey(ITEMS), loaded([ITEM(0), ITEM(1)])],
      [rowKey(ITEM(1)), loaded([PART_NAME])],
    ]);
    const visible = flattenRows([OBJECT], expanded, (key) => children.get(key));

    const owners = visible.map((line) => (line.kind === "row" ? line.owner : "more"));
    expect(owners).toEqual([null, "0x9b67e9f6", "0x9b67e9f6", null, null, "0x0000beef"]);
  });

  it("ignores an expansion on a row nothing can sit under", () => {
    const children = new Map([[rowKey(OBJECT), loaded([SIZE])]]);
    const expanded = new Set([rowKey(OBJECT), rowKey(SIZE)]);
    const visible = flattenRows([OBJECT], expanded, (key) => children.get(key));

    expect(visible[1]).toMatchObject({ key: `${ENTRY}:0000000a`, expanded: false, loading: false });
  });
});

describe("canExpand and childCount", () => {
  it("opens a struct, a container and a map that hold something, and a present optional", () => {
    expect(canExpand(OBJECT)).toBe(true);
    expect(canExpand(ITEMS)).toBe(true);
    expect(canExpand(row({ value: { type: "optional", present: true, itemKind: "f32" } }))).toBe(
      true,
    );
  });

  it("keeps a leaf, a null, an empty container and an absent optional shut", () => {
    expect(canExpand(SIZE)).toBe(false);
    expect(canExpand(row({ value: { type: "null" } }))).toBe(false);
    expect(canExpand(row({ value: { type: "container", len: 0, itemKind: "u8" } }))).toBe(false);
    expect(canExpand(row({ value: { type: "optional", present: false, itemKind: "u8" } }))).toBe(
      false,
    );
  });

  it("counts the records under a patch target", () => {
    expect(childCount(row({ node: "target", value: { type: "records", len: 4 } }))).toBe(4);
  });
});

describe("fieldHash", () => {
  it("reads the field a property path ends in, at the root and under a segment", () => {
    expect(fieldHash("9c4e1b02")).toBe("0x9c4e1b02");
    expect(fieldHash("0000000b[1].1a2b3c4d")).toBe("0x1a2b3c4d");
    expect(fieldHash('0000000b{"weapon"}.deadbeef')).toBe("0xdeadbeef");
  });
});

describe("mergePages", () => {
  const page = (rows: BinRow[], total: number) => ({ data: { rows, total } });
  const failed: AppError = { code: "BIN_NODE_NOT_FOUND", address: `${ENTRY}:0000000b` };

  it("is nothing until the first page answers", () => {
    expect(mergePages([{}])).toBeUndefined();
    expect(mergePages([])).toBeUndefined();
  });

  it("concatenates the answered pages in order and keeps the total", () => {
    const merged = mergePages([page([ITEM(0), ITEM(1)], 3), page([ITEM(2)], 3)]);

    expect(merged).toEqual({
      rows: [ITEM(0), ITEM(1), ITEM(2)],
      total: 3,
      pending: false,
      error: undefined,
    });
  });

  it("ends the rows at a page that has not answered and reads as pending", () => {
    const merged = mergePages([page([ITEM(0)], 3), {}, page([ITEM(2)], 3)]);

    expect(merged).toMatchObject({ rows: [ITEM(0)], total: 3, pending: true });
  });

  it("carries a failed page's error, with nothing under it when it was the first", () => {
    expect(mergePages([{ error: failed }])).toEqual({
      rows: [],
      total: 0,
      pending: false,
      error: failed,
    });
  });
});

describe("pagesWanted", () => {
  it("asks for one page past what a whole number of pages answered", () => {
    expect(pagesWanted(0)).toBe(1);
    expect(pagesWanted(PAGE_SIZE)).toBe(2);
    expect(pagesWanted(PAGE_SIZE * 2)).toBe(3);
  });
});

describe("isUnder", () => {
  it("holds for a node and what sits under it, and not for a sibling sharing a prefix", () => {
    expect(isUnder("0x1:", "0x1:aaaaaaaa[3]")).toBe(true);
    expect(isUnder("0x1:aaaaaaaa", "0x1:aaaaaaaa.bbbbbbbb")).toBe(true);
    expect(isUnder("0x1:aaaaaaaa[3]", "0x1:aaaaaaaa[30]")).toBe(false);
    expect(isUnder("0x1:aaaaaaaa", "0x2:aaaaaaaa")).toBe(false);
  });

  it("keeps an object's rows and its patch target's records apart", () => {
    expect(isUnder("0x1:#", "0x1:#3.aaaaaaaa")).toBe(true);
    expect(isUnder("0x1:#1", "0x1:#10")).toBe(false);
    expect(isUnder("0x1:", "0x1:#3")).toBe(false);
    expect(isUnder("0x1:#", "0x1:aaaaaaaa")).toBe(false);
  });
});

describe("ancestorKeys", () => {
  it("walks a field path down to the row, the object's own key first", () => {
    expect(ancestorKeys(`${ENTRY}:0000000a[3].0000000b`)).toEqual([
      `${ENTRY}:`,
      `${ENTRY}:0000000a`,
      `${ENTRY}:0000000a[3]`,
      `${ENTRY}:0000000a[3].0000000b`,
    ]);
  });

  it("walks a record path down from its target, the record's own key second", () => {
    expect(ancestorKeys(`${ENTRY}:#12[3].0000000b`)).toEqual([
      `${ENTRY}:#`,
      `${ENTRY}:#12`,
      `${ENTRY}:#12[3]`,
      `${ENTRY}:#12[3].0000000b`,
    ]);
    expect(ancestorKeys(`${ENTRY}:#`)).toEqual([`${ENTRY}:#`]);
  });

  it("answers the object's own key for the object itself", () => {
    expect(ancestorKeys(`${ENTRY}:`)).toEqual([`${ENTRY}:`]);
  });

  it("walks a map key, bare and quoted, as one segment", () => {
    expect(ancestorKeys(`${ENTRY}:0000000a{7}.0000000b`)).toEqual([
      `${ENTRY}:`,
      `${ENTRY}:0000000a`,
      `${ENTRY}:0000000a{7}`,
      `${ENTRY}:0000000a{7}.0000000b`,
    ]);
    expect(ancestorKeys(`${ENTRY}:0000000a{"we}ird"}`)).toEqual([
      `${ENTRY}:`,
      `${ENTRY}:0000000a`,
      `${ENTRY}:0000000a{"we}ird"}`,
    ]);
  });

  /* Every ancestor opens, so a reveal of a row nobody can reach still opens what it can. */
  it("answers what it reached for a path it cannot read, the key itself last", () => {
    expect(ancestorKeys(`${ENTRY}:0000000a[3`)).toEqual([
      `${ENTRY}:`,
      `${ENTRY}:0000000a`,
      `${ENTRY}:0000000a[3`,
    ]);
  });
});

describe("nameColumns", () => {
  const line = (row: BinRow, depth = 0): RowLine => ({
    kind: "row",
    key: rowKey(row),
    row,
    depth,
    expanded: false,
    loading: false,
    owner: null,
    parent: null,
    index: 0,
  });

  const noTag = () => null;

  it("widens the column for the longest property name it holds", () => {
    const long = row({ name: "mConditionFloatPairDataList" });

    expect(nameColumns([line(long)], noTag)).toBeGreaterThan(nameColumns([line(SIZE)], noTag));
  });

  it("leaves the column alone for an object and an element, which sit outside it", () => {
    const element = row({ node: "element", name: "[0]", path: "0000000a[0]" });
    const bare = nameColumns([line(SIZE)], noTag);

    expect(nameColumns([line(SIZE), line(OBJECT), line(element, 4)], noTag)).toBe(bare);
  });
});

describe("revealPage", () => {
  const LIST = `${ENTRY}:0000000b`;
  const ancestors = [`${ENTRY}:`, LIST, `${LIST}[700]`];
  const items = (count: number) => Array.from({ length: count }, (_, index) => ITEM(index));
  const answered =
    (pages: Record<string, LoadedChildren>): ((key: string) => LoadedChildren | undefined) =>
    (key) =>
      pages[key];

  it("asks the level whose answered rows end before the next key down for its next page", () => {
    const childrenOf = answered({
      [`${ENTRY}:`]: { rows: [SIZE, ITEMS], total: 2, pending: false },
      [LIST]: { rows: items(PAGE_SIZE), total: 900, pending: false },
    });
    expect(revealPage(ancestors, childrenOf)).toEqual({ parent: LIST, loaded: PAGE_SIZE });
  });

  it("owes nothing once every level holds its next key", () => {
    const childrenOf = answered({
      [`${ENTRY}:`]: { rows: [SIZE, ITEMS], total: 2, pending: false },
      [LIST]: { rows: items(900), total: 900, pending: false },
    });
    expect(revealPage(ancestors, childrenOf)).toBeNull();
  });

  it("gives up on a key no page of its level holds", () => {
    const childrenOf = answered({
      [`${ENTRY}:`]: { rows: [SIZE, ITEMS], total: 2, pending: false },
      [LIST]: { rows: items(3), total: 3, pending: false },
    });
    expect(revealPage(ancestors, childrenOf)).toBeNull();
  });
});

describe("revealLanding", () => {
  const OPTION = row({
    path: "0000000d",
    name: "rate",
    kind: "option",
    value: { type: "float", value: 2 },
  });

  function landing(expanded: string[], pages: Map<string, LoadedChildren>, key: string) {
    const lines = flattenRows([OBJECT], new Set(expanded), (at) => pages.get(at));
    const lineOf = (at: string) =>
      lines.find((each): each is RowLine => each.kind === "row" && each.key === at);
    return revealLanding(ancestorKeys(key), lineOf, (at) => pages.get(at));
  }

  it("lands on the row asked for once every level above it has answered", () => {
    const pages = new Map([
      [rowKey(OBJECT), loaded([SIZE, ITEMS])],
      [rowKey(ITEMS), loaded([ITEM(0), ITEM(1), ITEM(2)])],
    ]);
    const key = rowKey(ITEM(1));

    expect(landing([rowKey(OBJECT), rowKey(ITEMS)], pages, key)).toBe(key);
  });

  it("waits while a level above has not answered", () => {
    const pages = new Map([[rowKey(OBJECT), loaded([SIZE, ITEMS])]]);

    expect(landing([rowKey(OBJECT), rowKey(ITEMS)], pages, rowKey(ITEM(1)))).toBeNull();
  });

  it("lands on an option that draws its leaf for the `[0]` under it", () => {
    const pages = new Map([[rowKey(OBJECT), loaded([SIZE, OPTION])]]);
    const key = `${rowKey(OPTION)}[0]`;

    expect(landing([rowKey(OBJECT), rowKey(OPTION), key], pages, key)).toBe(rowKey(OPTION));
  });

  it("lands on the deepest row above a key no answered page holds", () => {
    const pages = new Map([[rowKey(OBJECT), loaded([SIZE])]]);

    expect(landing([rowKey(OBJECT)], pages, `${ENTRY}:0000000f`)).toBe(rowKey(OBJECT));
  });
});

describe("guideBlocks and lineParent", () => {
  it("names each guide of a line by the row at that depth, the object first", () => {
    expect(guideBlocks(`${ENTRY}:0000000b[3]`, 3)).toEqual([
      `${ENTRY}:`,
      `${ENTRY}:0000000b`,
      `${ENTRY}:0000000b[3]`,
    ]);
    expect(guideBlocks(null, 0)).toEqual([]);
  });

  it("hangs a row under the row it is a child of, and a more line under its holder", () => {
    const child: RowLine = {
      kind: "row",
      key: rowKey(ITEM(1)),
      row: ITEM(1),
      depth: 2,
      expanded: false,
      loading: false,
      owner: null,
      parent: ITEMS,
      index: 1,
    };
    expect(lineParent(child)).toBe(rowKey(ITEMS));
    expect(lineParent({ ...child, depth: 0, parent: null })).toBeNull();
    expect(
      lineParent({
        kind: "more",
        key: `${rowKey(ITEMS)}:more`,
        parent: rowKey(ITEMS),
        depth: 2,
        loaded: 500,
        total: 900,
        pending: false,
      }),
    ).toBe(rowKey(ITEMS));
  });
});

describe("a repeated map key", () => {
  it("walks down through a repeat as one segment", () => {
    expect(ancestorKeys(`${ENTRY}:0000000b{2acd4eca}#1.0000000c`)).toEqual([
      `${ENTRY}:`,
      `${ENTRY}:0000000b`,
      `${ENTRY}:0000000b{2acd4eca}#1`,
      `${ENTRY}:0000000b{2acd4eca}#1.0000000c`,
    ]);
  });

  it("keeps a repeat's rows apart from the first entry's", () => {
    const first = `${ENTRY}:0000000b{2acd4eca}`;
    expect(isUnder(first, `${first}#1.0000000c`)).toBe(false);
    expect(isUnder(`${first}#1`, `${first}#1.0000000c`)).toBe(true);
  });

  it("marks the later entries of a key and not the first", () => {
    expect(repeatsKey({ node: "entry", path: "0000000b{2acd4eca}#1" })).toBe(true);
    expect(repeatsKey({ node: "entry", path: "0000000b{2acd4eca}" })).toBe(false);
    expect(repeatsKey({ node: "entry", path: '0000000b{"a}#1"}' })).toBe(false);
  });
});
