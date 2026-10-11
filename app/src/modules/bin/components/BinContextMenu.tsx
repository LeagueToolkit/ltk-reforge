/**
 * The tree's row menu: the copies of what a row carries.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/tree/components/BinContextMenu.tsx`. Cut:
 * the edits, the links, the references, the curve dock, the declarations and the object items,
 * and the value hash a string resolves to, which needs the link targets.
 */
import { CopyIcon, HashIcon, PathIcon } from "@phosphor-icons/react";

import { ContextMenu } from "@/components";
import { m } from "@/i18n";
import type { BinValue } from "@/lib/bindings.gen";
import { copyText } from "@/utils";

import { useValueMark } from "../hooks/useValueMarks";
import { fieldHash, type VisibleRow } from "../rows";
import { markText } from "../valueRows";

interface BinContextMenuProps {
  /** The line the menu was opened on. Null while it has never been opened. */
  line: VisibleRow | null;
  /** The name of the object an entry hash addresses, for the path a row copies. */
  objectName: (entry: string) => string;
}

/**
 * The list's one menu, aimed at whichever row opened it.
 *
 * Copy path is the object's path and the property path joined on a colon, and the object's
 * path alone for an object row and a patch target row, as LTK Manager spells it.
 */
export function BinContextMenu({ line, objectName }: BinContextMenuProps) {
  const mark = useValueMark(line?.kind === "row" ? line.key : undefined);
  if (line?.kind !== "row") return null;

  const { row } = line;
  const property = row.node === "property";
  const path =
    row.node === "object" || row.node === "target"
      ? row.name
      : `${objectName(row.entry)}:${row.label}`;
  const struct = row.value.type === "struct" ? row.value : null;
  const structName = struct?.class ?? null;
  const valueText = readableValue(row.value) ?? markText(mark);
  const valueHash = linkedValueHash(row.value);

  return (
    <ContextMenu.Content className="w-56">
      <ContextMenu.Item icon={<PathIcon />} onClick={() => copyText(path)}>
        {m.bin_copy_path_action()}
      </ContextMenu.Item>
      {property && !row.unnamed && (
        <ContextMenu.Item icon={<CopyIcon />} onClick={() => copyText(row.name)}>
          {m.bin_copy_name_action()}
        </ContextMenu.Item>
      )}
      {property && (
        <ContextMenu.Item icon={<HashIcon />} onClick={() => copyText(fieldHash(row.path))}>
          {m.bin_copy_field_hash_action()}
        </ContextMenu.Item>
      )}
      {valueText !== null && (
        <ContextMenu.Item icon={<CopyIcon />} onClick={() => copyText(valueText)}>
          {m.bin_copy_value_action()}
        </ContextMenu.Item>
      )}
      {valueHash !== null && (
        <ContextMenu.Item icon={<HashIcon />} onClick={() => copyText(valueHash)}>
          {m.bin_copy_value_hash_action()}
        </ContextMenu.Item>
      )}
      {structName !== null && (
        <ContextMenu.Item icon={<CopyIcon />} onClick={() => copyText(structName)}>
          {m.bin_copy_class_name_action()}
        </ContextMenu.Item>
      )}
      {struct !== null && (
        <ContextMenu.Item icon={<HashIcon />} onClick={() => copyText(struct.classHash)}>
          {m.bin_copy_class_hash_action()}
        </ContextMenu.Item>
      )}
    </ContextMenu.Content>
  );
}

/** The hash behind a hash or a link value, whether or not a table names it. */
function linkedValueHash(value: BinValue): string | null {
  switch (value.type) {
    case "hash":
    case "objectLink":
    case "wadChunkLink":
      return value.hash;
    default:
      return null;
  }
}

/**
 * The value as the one string a reader would take, or null where it reads as none.
 *
 * A container, a map, a struct and an optional draw a tally rather than a value, and a copy of
 * "22 items" is what nobody asked for.
 */
function readableValue(value: BinValue): string | null {
  switch (value.type) {
    case "bool":
      return String(value.value);
    case "integer":
      return value.text;
    case "float":
      return String(value.value);
    case "vector":
    case "matrix":
      return value.values.join(", ");
    case "color":
      return `#${[value.r, value.g, value.b, value.a].map(hexByte).join("")}`;
    case "string":
      return value.value;
    case "hash":
    case "objectLink":
      return value.name;
    case "wadChunkLink":
      return value.path;
    default:
      return null;
  }
}

function hexByte(channel: number): string {
  return channel.toString(16).padStart(2, "0").toUpperCase();
}
