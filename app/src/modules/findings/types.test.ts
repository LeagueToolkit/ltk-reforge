import { describe, expect, it } from "vitest";

import type { Finding } from "@/lib/bindings.gen";

import { hashesIn, isHash } from "./hashes";
import { fieldsOf, parseFieldPath, pathText, shapeText, typeFinding } from "./types";

const names: Record<string, string> = {
  "14e6a52c": "VfxVector3fBase",
  bc037de7: "dynamics",
};
const spell = (hash: string) => names[hash] ?? hash;

describe("shapeText", () => {
  it("writes the class beside a pointer and an embed, as the finding's message does", () => {
    expect(shapeText({ kind: "Pointer", class: "14e6a52c" }, spell)).toBe(
      "Pointer<VfxVector3fBase>",
    );
    expect(shapeText({ kind: "Embed", class: "fe064c88" }, spell)).toBe("Embed<fe064c88>");
    expect(shapeText({ kind: "F32" }, spell)).toBe("F32");
  });

  it("writes a container's item and a map's key and value", () => {
    expect(shapeText({ kind: "List", item: "Pointer", class: "14e6a52c" }, spell)).toBe(
      "List<Pointer<VfxVector3fBase>>",
    );
    expect(shapeText({ kind: "Map", key: "Hash", item: "Embed", class: "14e6a52c" }, spell)).toBe(
      "Map<Hash, Embed<VfxVector3fBase>>",
    );
    expect(shapeText({ kind: "Option", item: "U8" }, spell)).toBe("Option<U8>");
  });

  it("leaves the class off a kind that holds no struct", () => {
    expect(shapeText({ kind: "List", item: "Hash", class: "14e6a52c" }, spell)).toBe("List<Hash>");
  });
});

describe("parseFieldPath", () => {
  it("reads fields, indexes and keys in the hash form", () => {
    expect(parseFieldPath('bc037de7[2].0000abcd{"a}b"}.12345678')).toEqual([
      { field: "bc037de7" },
      { index: 2 },
      { field: "0000abcd" },
      { key: '"a}b"' },
      { field: "12345678" },
    ]);
    expect(parseFieldPath("")).toEqual([]);
    expect(parseFieldPath("{9dbcac96}")).toEqual([{ key: "9dbcac96" }]);
  });

  it("reads a string key holding the path's own characters, as the app sends it cleaned", () => {
    // The form `places.rs`'s `a_string_map_key_reaches_the_window_cleaned_and_written_as_json`
    // pins on the backend.
    const at = 'bc037de7[3]{"a.b]c}\\"d\\u{202e}"}.0000abcd';
    const steps = parseFieldPath(at);
    expect(steps).toEqual([
      { field: "bc037de7" },
      { index: 3 },
      { key: '"a.b]c}\\"d\\u{202e}"' },
      { field: "0000abcd" },
    ]);
    expect(pathText(steps ?? [], (hash) => hash)).toBe(at);
  });

  it("refuses text of another form", () => {
    expect(parseFieldPath("not a path")).toBeNull();
    expect(parseFieldPath(".bc037de7")).toBeNull();
    expect(parseFieldPath("bc037de7{open")).toBeNull();
    expect(parseFieldPath("bc037de70000abcd")).toBeNull();
    expect(parseFieldPath("BC037DE7")).toBeNull();
    expect(parseFieldPath('bc037de7{"a"b}')).toBeNull();
    expect(parseFieldPath('bc037de7{"open}')).toBeNull();
  });

  it("names each field it can, and lists the fields once", () => {
    const steps = parseFieldPath("bc037de7[0].bc037de7.0000abcd") ?? [];
    expect(pathText(steps, spell)).toBe("dynamics[0].dynamics.0000abcd");
    expect(fieldsOf(steps)).toEqual(["bc037de7", "0000abcd"]);
  });
});

/** A finding of a code with these fields, as the run's view holds it. */
function finding(code: string, data: unknown): Finding {
  return {
    id: 1,
    code,
    severity: "error",
    channel: "run",
    stage: "check",
    subject: { kind: "run", id: "run", name: "run" },
    by: "types",
    record: null,
    message: "",
    data,
  };
}

const first = { wad: "Test.wad.client", path: "data/x.bin", entry: "9dbcac96" };

describe("typeFinding", () => {
  it("reads a mismatch's rows with their shapes", () => {
    const found = typeFinding(
      finding("check.type-mismatch", {
        values: 2,
        fields: [
          {
            class: "68dc32b6",
            field: "bc037de7",
            found: { kind: "Pointer", class: "fe064c88" },
            wants: { kind: "Pointer", class: "14e6a52c" },
            values: 2,
            first,
          },
        ],
      }),
    );
    expect(found?.list).toBe("mismatched");
    expect(found?.rows).toEqual([
      {
        class: "68dc32b6",
        field: "bc037de7",
        held: undefined,
        found: { kind: "Pointer", key: undefined, item: undefined, class: "fe064c88" },
        wants: { kind: "Pointer", key: undefined, item: undefined, class: "14e6a52c" },
        count: 2,
        first,
      },
    ]);
  });

  it("reads an undeclared class's rows, held by a field or as objects", () => {
    const found = typeFinding(
      finding("check.class-undeclared", {
        structs: 2,
        classes: [
          { held: "aaaaaaaa", structs: 1, first },
          { class: "68dc32b6", field: "bc037de7", held: "bbbbbbbb", structs: 1, first },
        ],
      }),
    );
    expect(found?.list).toBe("classes");
    expect(found?.rows.map((row) => [row.class, row.held, row.count])).toEqual([
      [undefined, "aaaaaaaa", 1],
      ["68dc32b6", "bbbbbbbb", 1],
    ]);
  });

  it("takes no other finding, and no data of another shape", () => {
    expect(typeFinding(finding("check.other", { fields: [] }))).toBeNull();
    expect(typeFinding(finding("check.field-undeclared", { fields: [{ field: "x" }] }))).toBeNull();
  });
});

describe("hashesIn", () => {
  it("takes the hashes under the keys that name one, and under a field that holds a struct", () => {
    const data = {
      fields: [{ class: "68dc32b6", field: "bc037de7", values: 2, first }],
      held_by: { class: "11111111", other: "22222222" },
      path: "0123456789abcdef",
      hash: "0123456789abcdef",
    };
    expect(hashesIn(data)).toEqual([
      "68dc32b6",
      "bc037de7",
      "9dbcac96",
      "11111111",
      "22222222",
      "0123456789abcdef",
    ]);
  });

  it("knows a hash by its eight or sixteen hex digits", () => {
    expect(isHash("bc037de7")).toBe(true);
    expect(isHash("0123456789abcdef")).toBe(true);
    expect(isHash("bc037de")).toBe(false);
    expect(isHash("Skin0")).toBe(false);
  });
});
