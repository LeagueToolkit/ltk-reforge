import type { Finding } from "@/lib/bindings.gen";

/** A type as the type check's findings hold it: the kinds by their names, the class by hash. */
export interface Shape {
  kind: string;
  /** A map's key kind. */
  key?: string;
  /** A container's or an option's item kind, a map's value kind. */
  item?: string;
  /** The class of an embed or a pointer, or of a container's items. */
  class?: string;
}

/** The kinds that hold a struct, as the library names them: their class is written beside. */
const STRUCT_KINDS: readonly string[] = ["Embed", "Pointer"];

/**
 * A shape as the finding's message writes it (`Shape::message` in the library):
 * `Map<Hash, Embed<Class>>`, the class beside the kind that holds a struct.
 */
export function shapeText(shape: Shape, className: (hash: string) => string): string {
  const item = (kind: string) =>
    shape.class !== undefined && STRUCT_KINDS.includes(kind)
      ? `${kind}<${className(shape.class)}>`
      : kind;
  if (shape.key !== undefined && shape.item !== undefined) {
    return `${shape.kind}<${shape.key}, ${item(shape.item)}>`;
  }
  if (shape.item !== undefined) {
    return `${shape.kind}<${item(shape.item)}>`;
  }
  return item(shape.kind);
}

/** One step of a value's path in its object: a field by hash, a list's index, a map's key. */
export type Step = { field: string } | { index: number } | { key: string };

/**
 * A value's path in the hash form `ltk_meta`'s `ValuePath` writes: a field as eight lowercase
 * hex digits, `.` before each field but the first, `[i]` an index, `{key}` a map entry. A key
 * is a number, a tuple, a hash in hex, or a string as JSON, which alone may hold `}`. Null for
 * text of another form, which a caller shows as it is.
 */
export function parseFieldPath(at: string): Step[] | null {
  const steps: Step[] = [];
  let i = 0;
  while (i < at.length) {
    const rest = at.slice(i);
    const field = (steps.length === 0 ? /^([0-9a-f]{8})/ : /^\.([0-9a-f]{8})/).exec(rest);
    if (field) {
      steps.push({ field: field[1] });
      i += field[0].length;
      continue;
    }
    const index = /^\[(\d+)\]/.exec(rest);
    if (index) {
      steps.push({ index: Number(index[1]) });
      i += index[0].length;
      continue;
    }
    if (rest.startsWith("{")) {
      const end = keyEnd(at, i + 1);
      if (end < 0) {
        return null;
      }
      steps.push({ key: at.slice(i + 1, end) });
      i = end + 1;
      continue;
    }
    return null;
  }
  return steps;
}

/**
 * Where a map key that starts at `from` ends: its closing `}`, right after a JSON string's
 * closing quote. -1 for none.
 */
function keyEnd(at: string, from: number): number {
  if (at[from] !== '"') {
    return at.indexOf("}", from);
  }
  let i = from + 1;
  while (i < at.length && at[i] !== '"') {
    i += at[i] === "\\" ? 2 : 1;
  }
  return at[i + 1] === "}" ? i + 1 : -1;
}

/** The fields a path names, each once. */
export function fieldsOf(steps: readonly Step[]): string[] {
  const fields = steps.flatMap((step) => ("field" in step ? [step.field] : []));
  return [...new Set(fields)];
}

/** A path for a person: each field by its name where one is given, else its hash. */
export function pathText(steps: readonly Step[], fieldName: (hash: string) => string): string {
  return steps
    .map((step, at) => {
      if ("field" in step) {
        return `${at > 0 ? "." : ""}${fieldName(step.field)}`;
      }
      return "index" in step ? `[${step.index}]` : `{${step.key}}`;
    })
    .join("");
}

/** Which of the type check's three lists a finding's rows are, by its code. */
export type TypeList = "mismatched" | "undeclared" | "classes";

const TYPE_LISTS: Partial<Record<string, TypeList>> = {
  "check.type-mismatch": "mismatched",
  "check.field-undeclared": "undeclared",
  "check.class-undeclared": "classes",
};

/** Where a check found a value: the WAD, the bin's path and the object's entry hash. */
export interface FirstPlace {
  wad: string;
  path: string;
  entry: string;
}

/** One row of a type check's finding, as the three findings hold their rows. */
export interface TypeRow {
  /** The class of the struct the field is on. Absent for objects of an undeclared class. */
  class?: string;
  /** The field. Absent for objects of an undeclared class. */
  field?: string;
  /** The undeclared class of the structs held. */
  held?: string;
  /** The shape written, for a value of another type. */
  found?: Shape;
  /** The shape the target build reads, for a value of another type. */
  wants?: Shape;
  /** How many values or structs. */
  count: number;
  /** The first of them. */
  first: FirstPlace;
}

/** A type check's finding: which list its rows are, and the rows in the finding's order. */
export interface TypeFinding {
  list: TypeList;
  rows: TypeRow[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function text(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function shapeOf(value: unknown): Shape | undefined {
  if (!isRecord(value) || typeof value.kind !== "string") {
    return undefined;
  }
  return {
    kind: value.kind,
    key: text(value.key),
    item: text(value.item),
    class: text(value.class),
  };
}

function rowOf(value: unknown): TypeRow | null {
  if (!isRecord(value) || !isRecord(value.first)) {
    return null;
  }
  const { wad, path, entry } = value.first;
  const count = value.values ?? value.structs;
  if (typeof wad !== "string" || typeof path !== "string" || typeof entry !== "string") {
    return null;
  }
  if (typeof count !== "number") {
    return null;
  }
  return {
    class: text(value.class),
    field: text(value.field),
    held: text(value.held),
    found: shapeOf(value.found),
    wants: shapeOf(value.wants),
    count,
    first: { wad, path, entry },
  };
}

/** A type check's finding's rows. Null for another finding, or data of another shape. */
export function typeFinding(finding: Finding): TypeFinding | null {
  const list = TYPE_LISTS[finding.code];
  const data: unknown = finding.data;
  if (!list || !isRecord(data)) {
    return null;
  }
  const items = list === "classes" ? data.classes : data.fields;
  if (!Array.isArray(items)) {
    return null;
  }
  const rows = items.map(rowOf);
  return rows.every((row) => row !== null) ? { list, rows } : null;
}

/** The hashes a type check's rows name: classes, fields, the classes held and first entries. */
export function rowHashes(rows: readonly TypeRow[]): string[] {
  const hashes = rows.flatMap((row) => [
    row.class,
    row.field,
    row.held,
    row.found?.class,
    row.wants?.class,
    row.first.entry,
  ]);
  return hashes.filter((hash): hash is string => hash !== undefined);
}
