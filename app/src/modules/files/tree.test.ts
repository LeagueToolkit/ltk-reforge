import { describe, expect, it } from "vitest";

import type { FileRow, Mark, WadFiles } from "@/lib/bindings.gen";

import {
  buildTree,
  HASHED_FOLDER,
  sizeLabel,
  sourcesOf,
  type TreeFolder,
  type TreeNode,
} from "./tree";

function row(path: string | null, size: number, mark: Mark = "same", kind = "texture"): FileRow {
  return { hash: "0123456789abcdef", path, content: "00".repeat(32), size, kind, mark };
}

const wad: WadFiles = {
  name: "Ahri.wad.client",
  files: [
    row("assets/characters/ahri/skins/base/ahri.tex", 100),
    row("assets/characters/ahri/skins/base/ahri.skn", 50, "changed", "simple-skin"),
    row("data/characters/ahri/skins/skin0.bin", 10, "added", "property-bin"),
    row("assets/characters/ahri/skins/base/old.tex", 7, "removed"),
    row(null, 5),
  ],
};

const all = { text: "", changedOnly: false };

/** A folder's children by name. */
function names(node: TreeFolder): string[] {
  return node.children.map((child) => child.name);
}

function child(node: TreeFolder, name: string): TreeNode {
  const found = node.children.find((c) => c.name === name);
  if (!found) {
    throw new Error(`no ${name} in ${node.key}: ${names(node).join(", ")}`);
  }
  return found;
}

function folderAt(root: TreeFolder, ...path: string[]): TreeFolder {
  let node: TreeNode = root;
  for (const name of path) {
    if (node.kind !== "folder") {
      throw new Error(`${node.key} is a file`);
    }
    node = child(node, name);
  }
  if (node.kind !== "folder") {
    throw new Error(`${node.key} is a file`);
  }
  return node;
}

describe("buildTree", () => {
  it("makes a folder of each part of the paths, folders first and the hashed one last", () => {
    const [root] = buildTree([wad], all);
    expect(root.name).toBe("Ahri.wad.client");
    expect(names(root)).toEqual(["assets", "data", HASHED_FOLDER]);
    const base = folderAt(root, "assets", "characters", "ahri", "skins", "base");
    expect(names(base)).toEqual(["ahri.skn", "ahri.tex", "old.tex"]);
  });

  it("puts a file named by its hash alone under the hashed folder, by its hash", () => {
    const [root] = buildTree([wad], all);
    expect(names(folderAt(root, HASHED_FOLDER))).toEqual(["0123456789abcdef"]);
  });

  it("counts the files and sums the bytes under each folder, leaving out the removed", () => {
    const [root] = buildTree([wad], all);
    expect([root.files, root.bytes]).toEqual([4, 165]);
    const base = folderAt(root, "assets", "characters", "ahri", "skins", "base");
    expect([base.files, base.bytes]).toEqual([2, 150]);
  });

  it("shows only the changed, added and removed files when asked", () => {
    const [root] = buildTree([wad], { text: "", changedOnly: true });
    const base = folderAt(root, "assets", "characters", "ahri", "skins", "base");
    expect(names(base)).toEqual(["ahri.skn", "old.tex"]);
    expect(names(root)).toEqual(["assets", "data"]);
    expect([root.files, root.bytes]).toEqual([2, 60]);
  });

  it("filters by path and by kind, and leaves out a WAD the filter empties", () => {
    const other: WadFiles = { name: "Other.wad.client", files: [row("a/b.dds", 1)] };
    const byKind = buildTree([wad, other], { text: "PROPERTY", changedOnly: false });
    expect(byKind.map((root) => root.name)).toEqual(["Ahri.wad.client"]);
    expect(names(byKind[0])).toEqual(["data"]);
    const byPath = buildTree([wad, other], { text: "b.dds", changedOnly: false });
    expect(byPath.map((root) => root.name)).toEqual(["Other.wad.client"]);
  });

  it("keeps an empty WAD while nothing narrows the tree", () => {
    const empty: WadFiles = { name: "Empty.wad.client", files: [] };
    expect(buildTree([empty], all).map((root) => root.files)).toEqual([0]);
  });

  it("shows only the files of one provenance fact when asked", () => {
    const sourced: WadFiles = {
      name: "Ahri.wad.client",
      files: [
        { ...row("a/game.tex", 10), source: "install" },
        { ...row("a/own.tex", 20), source: "own" },
        row("a/none.tex", 30),
      ],
    };
    const [root] = buildTree([sourced], { ...all, source: "install" });
    expect(names(folderAt(root, "a"))).toEqual(["game.tex"]);
    const [every] = buildTree([sourced], { ...all, source: null });
    expect(names(folderAt(every, "a"))).toEqual(["game.tex", "none.tex", "own.tex"]);
  });
});

describe("sourcesOf", () => {
  it("lists each fact the files carry once, in the order the library declares them", () => {
    const files = [
      { ...row("a", 1), source: "own" },
      { ...row("b", 1), source: "history" },
      { ...row("c", 1), source: "own" },
      { ...row("d", 1), source: "install" },
      row("e", 1),
    ];
    expect(sourcesOf([{ name: "W.wad.client", files }])).toEqual(["install", "history", "own"]);
    expect(sourcesOf([wad])).toEqual([]);
  });
});

describe("sizeLabel", () => {
  it("writes bytes under a kilobyte whole and larger sizes in the unit that fits", () => {
    expect(sizeLabel(38)).toBe("38 B");
    expect(sizeLabel(1500)).toBe("1.5 KB");
    expect(sizeLabel(2_500_000)).toBe("2.5 MB");
    expect(sizeLabel(3_000_000_000)).toBe("3.0 GB");
  });

  it("writes the unit words it is given", () => {
    const units = { bytes: "o", kilo: "ko", mega: "Mo", giga: "Go" };
    expect(sizeLabel(38, units)).toBe("38 o");
    expect(sizeLabel(2_500_000, units)).toBe("2.5 Mo");
  });
});
