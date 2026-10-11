import {
  ArchiveIcon,
  CaretDownIcon,
  CaretRightIcon,
  CubeIcon,
  FileCodeIcon,
  FileIcon,
  FileTextIcon,
  FileXIcon,
  FolderIcon,
  FolderOpenIcon,
  ImageIcon,
  SpeakerHighIcon,
} from "@phosphor-icons/react";
import { type ReactNode, useState } from "react";

import { Tooltip } from "@/components";
import { m } from "@/i18n";
import type { FileRow, Mark } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { sizeLabel, type TreeFile, type TreeFolder } from "../tree";

const BIN_KINDS = ["property-bin", "property-bin-override", "ritobin-text", "ritobin-json"];
const IMAGE_KINDS = ["texture", "texture-dds", "png", "jpeg", "gif", "bmp", "tga", "tiff", "webp"];
const MODEL_KINDS = [
  "animation",
  "skeleton",
  "simple-skin",
  "static-mesh-binary",
  "static-mesh-ascii",
];
const AUDIO_KINDS = ["wwise-bank", "wwise-package", "wav", "wem", "ogg"];
const TEXT_KINDS = ["text", "json", "xml", "ini", "troy", "markdown", "script"];

/** The icon of a file's kind, as `reforge-magic` names kinds. */
function kindIcon(kind: string): ReactNode {
  const className = "h-3.5 w-3.5 shrink-0 text-surface-400";
  if (kind === "missing") {
    return <FileXIcon weight="bold" className={twMerge(className, "text-danger-text")} />;
  }
  if (BIN_KINDS.includes(kind)) {
    return <FileCodeIcon weight="bold" className={className} />;
  }
  if (IMAGE_KINDS.includes(kind)) {
    return <ImageIcon weight="bold" className={className} />;
  }
  if (MODEL_KINDS.includes(kind)) {
    return <CubeIcon weight="bold" className={className} />;
  }
  if (AUDIO_KINDS.includes(kind)) {
    return <SpeakerHighIcon weight="bold" className={className} />;
  }
  if (TEXT_KINDS.includes(kind)) {
    return <FileTextIcon weight="bold" className={className} />;
  }
  return <FileIcon weight="bold" className={className} />;
}

const markClass: Record<Exclude<Mark, "same">, string> = {
  changed: "bg-warning",
  added: "bg-success",
  removed: "bg-danger",
};

/**
 * A file's mark: a coloured dot, its word on hover, and against what when that is another run.
 * None for a file the same as before.
 */
function MarkDot({ mark, againstRun }: { mark: Mark; againstRun: boolean }) {
  if (mark === "same") {
    return null;
  }
  const word = m[`runs_mark.${mark}.label`]();
  const label = againstRun ? m.compare_mark_hint({ mark: word }) : word;
  return (
    <Tooltip content={label}>
      <span
        role="img"
        aria-label={label}
        className={twMerge("h-2 w-2 shrink-0 rounded-full", markClass[mark])}
      />
    </Tooltip>
  );
}

/** Indents a row by its depth. */
function depthStyle(depth: number) {
  return { paddingLeft: `${depth * 16 + 8}px` };
}

const rowClass =
  "flex w-full cursor-pointer items-center gap-2 rounded-md py-1 pr-2 text-left text-xs select-none hover:bg-surface-veil";

interface FileTreeProps {
  roots: TreeFolder[];
  /** Every folder open, while a filter narrows the tree. */
  allOpen: boolean;
  selected: FileRow | null;
  onPick: (row: FileRow) => void;
  /** Whether the marks are against another run's files, not the stage before. */
  againstRun?: boolean;
}

/**
 * A stage's files as LTK Manager's Workshop shows a WAD: each WAD a root, folders with a count,
 * files with their kind, size and mark. A WAD opens at first and a folder closed, so a WAD of
 * thousands of files draws its rows as they are opened.
 */
export function FileTree({ roots, allOpen, selected, onPick, againstRun = false }: FileTreeProps) {
  // The keys whose open state differs from the default: a root open, a folder closed.
  const [flipped, setFlipped] = useState<Set<string>>(() => new Set());
  const isOpen = (node: TreeFolder, depth: number) =>
    allOpen || (depth === 0) !== flipped.has(node.key);
  const flip = (key: string) =>
    setFlipped((keys) => {
      const next = new Set(keys);
      if (!next.delete(key)) {
        next.add(key);
      }
      return next;
    });

  const file = (node: TreeFile, depth: number) => (
    <li key={node.key}>
      <button
        type="button"
        aria-pressed={selected === node.row}
        onClick={() => onPick(node.row)}
        className={twMerge(rowClass, "aria-pressed:bg-accent-500/10 aria-pressed:text-surface-50")}
        style={depthStyle(depth)}
        data-ui="FileTree:file"
      >
        {kindIcon(node.row.kind)}
        <span
          className={twMerge(
            "min-w-0 flex-1 truncate font-mono text-surface-200",
            node.row.mark === "removed" && "text-surface-400 line-through",
          )}
        >
          {node.name}
        </span>
        <MarkDot mark={node.row.mark} againstRun={againstRun} />
        <span className="shrink-0 text-surface-400 tabular-nums">{sizeLabel(node.row.size)}</span>
      </button>
    </li>
  );

  const folder = (node: TreeFolder, depth: number): ReactNode => {
    const open = isOpen(node, depth);
    const Caret = open ? CaretDownIcon : CaretRightIcon;
    const Icon = depth === 0 ? ArchiveIcon : open ? FolderOpenIcon : FolderIcon;
    return (
      <li key={node.key}>
        <button
          type="button"
          aria-expanded={open}
          onClick={() => flip(node.key)}
          className={rowClass}
          style={depthStyle(depth)}
          data-ui="FileTree:folder"
        >
          <Caret weight="bold" className="h-3 w-3 shrink-0 text-surface-400" />
          <Icon weight="bold" className="h-3.5 w-3.5 shrink-0 text-surface-300" />
          <span
            className={twMerge(
              "min-w-0 flex-1 truncate",
              depth === 0 ? "font-medium text-surface-50" : "font-mono text-surface-200",
            )}
          >
            {node.name}
          </span>
          <span className="shrink-0 text-surface-400 tabular-nums">
            {m.runs_files_count({ count: node.files })}
            {depth === 0 && ` · ${sizeLabel(node.bytes)}`}
          </span>
        </button>
        {open && (
          <ul>
            {node.children.map((child) =>
              child.kind === "folder" ? folder(child, depth + 1) : file(child, depth + 1),
            )}
          </ul>
        )}
      </li>
    );
  };

  return <ul data-ui="FileTree">{roots.map((root) => folder(root, 0))}</ul>;
}
