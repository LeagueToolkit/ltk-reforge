import { m } from "@/i18n";
import type { FileRow, WadFiles } from "@/lib/bindings.gen";

/** The folder the files named by their hash alone sit under, last, as LTK Manager shows them. */
export const HASHED_FOLDER = "?";

/** A file of the tree: its name, the last part of its path, or its hash. */
export interface TreeFile {
  kind: "file";
  key: string;
  name: string;
  row: FileRow;
}

/** A WAD or a folder of the tree, with the files under it counted and their bytes summed. */
export interface TreeFolder {
  kind: "folder";
  key: string;
  name: string;
  children: TreeNode[];
  /** The files under it the stage holds: a removed one is shown, and not counted. */
  files: number;
  /** Their bytes. */
  bytes: number;
}

export type TreeNode = TreeFolder | TreeFile;

/** What the tree shows of a stage's files. */
export interface TreeFilter {
  /** Words a file's path, hash or kind holds, in any case. */
  text: string;
  /** Only the files changed, added or removed against the stage before. */
  changedOnly: boolean;
  /** Only the files of this provenance fact, for files that carry one. */
  source?: string | null;
}

/**
 * The provenance facts in the order the library declares them (`BytesOrigin`): bytes the
 * install holds, bytes the game shipped at earlier builds only, the mod's own.
 */
export const SOURCE_FACTS = ["install", "history", "own"] as const;

/** A provenance fact the app has words for. */
export type SourceFact = (typeof SOURCE_FACTS)[number];

/** Whether a fact is one the app has words for. */
export function isSourceFact(fact: string): fact is SourceFact {
  return (SOURCE_FACTS as readonly string[]).includes(fact);
}

/** Orders by provenance fact as the library declares them, a fact it does not name last. */
export function bySourceFact(a: string, b: string): number {
  const place = (fact: string) =>
    isSourceFact(fact) ? SOURCE_FACTS.indexOf(fact) : SOURCE_FACTS.length;
  return place(a) - place(b) || (a < b ? -1 : a > b ? 1 : 0);
}

/** The provenance facts a stage's files carry, each once, in the facts' order. */
export function sourcesOf(wads: WadFiles[]): string[] {
  const facts = new Set<string>();
  for (const row of wads.flatMap((wad) => wad.files)) {
    if (row.source) {
      facts.add(row.source);
    }
  }
  return [...facts].sort(bySourceFact);
}

/** Whether a filter is narrowing the tree. */
export function filtering(filter: TreeFilter): boolean {
  return filter.changedOnly || filter.text.trim() !== "" || filter.source != null;
}

/** Whether a file passes a filter. */
export function shown(row: FileRow, filter: TreeFilter): boolean {
  if (filter.changedOnly && row.mark === "same") {
    return false;
  }
  if (filter.source != null && row.source !== filter.source) {
    return false;
  }
  const words = filter.text.trim().toLowerCase();
  if (!words) {
    return true;
  }
  return [row.path ?? row.hash, row.kind].some((text) => text.toLowerCase().includes(words));
}

function folder(key: string, name: string): TreeFolder {
  return { kind: "folder", key, name, children: [], files: 0, bytes: 0 };
}

/** Folders first, the hashed folder last among them, then files, each by name. */
function order(a: TreeNode, b: TreeNode): number {
  if (a.kind !== b.kind) {
    return a.kind === "folder" ? -1 : 1;
  }
  const hashed = (node: TreeNode) => node.kind === "folder" && node.name === HASHED_FOLDER;
  if (hashed(a) !== hashed(b)) {
    return hashed(a) ? 1 : -1;
  }
  return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
}

/** Sorts a folder's children, and counts the files and bytes under it. */
function settle(node: TreeFolder): void {
  node.files = 0;
  node.bytes = 0;
  for (const child of node.children) {
    if (child.kind === "folder") {
      settle(child);
      node.files += child.files;
      node.bytes += child.bytes;
    } else if (child.row.mark !== "removed") {
      node.files += 1;
      node.bytes += child.row.size;
    }
  }
  node.children.sort(order);
}

/**
 * A stage's files as a tree: each WAD a root, a folder per part of the paths, the files named
 * by their hash alone under the hashed folder. A WAD with no file the filter passes is left
 * out while the filter narrows.
 */
export function buildTree(wads: WadFiles[], filter: TreeFilter): TreeFolder[] {
  const roots: TreeFolder[] = [];
  for (const wad of wads) {
    const root = folder(wad.name, wad.name);
    const folders = new Map<string, TreeFolder>([[root.key, root]]);
    for (const row of wad.files.filter((file) => shown(file, filter))) {
      const parts = row.path ? row.path.split("/") : [HASHED_FOLDER, row.hash];
      let parent = root;
      for (const part of parts.slice(0, -1)) {
        const key = `${parent.key}/${part}`;
        let next = folders.get(key);
        if (!next) {
          next = folder(key, part);
          folders.set(key, next);
          parent.children.push(next);
        }
        parent = next;
      }
      const name = parts[parts.length - 1];
      parent.children.push({ kind: "file", key: `${parent.key}/${name}`, name, row });
    }
    settle(root);
    if (root.children.length > 0 || !filtering(filter)) {
      roots.push(root);
    }
  }
  return roots;
}

/**
 * The keys of the folders down to a file, its WAD's first and its own folder's last. Null for a
 * file the tree does not show.
 */
export function foldersTo(roots: readonly TreeFolder[], row: FileRow): string[] | null {
  const down = (node: TreeFolder): string[] | null => {
    for (const child of node.children) {
      if (child.kind === "file" && child.row === row) {
        return [node.key];
      }
      const below = child.kind === "folder" ? down(child) : null;
      if (below) {
        return [node.key, ...below];
      }
    }
    return null;
  };
  for (const root of roots) {
    const found = down(root);
    if (found) {
      return found;
    }
  }
  return null;
}

/** The words a size is written in: bytes, then each unit a thousand times the one before. */
export interface SizeUnits {
  bytes: string;
  kilo: string;
  mega: string;
  giga: string;
}

/** The size units in the person's language. */
export function sizeUnits(): SizeUnits {
  return {
    bytes: m.size_unit_b(),
    kilo: m.size_unit_kb(),
    mega: m.size_unit_mb(),
    giga: m.size_unit_gb(),
  };
}

/** A size for a person: bytes under a kilobyte, else the unit that keeps it under a thousand. */
export function sizeLabel(bytes: number, units: SizeUnits = sizeUnits()): string {
  if (bytes < 1000) {
    return `${bytes} ${units.bytes}`;
  }
  let value = bytes;
  let unit = units.bytes;
  for (const next of [units.kilo, units.mega, units.giga]) {
    if (value < 1000) {
      break;
    }
    value /= 1000;
    unit = next;
  }
  return `${value.toFixed(1)} ${unit}`;
}
