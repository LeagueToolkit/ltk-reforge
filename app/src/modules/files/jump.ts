import type { QueryKey } from "@tanstack/react-query";
import { createContext, useCallback, useState } from "react";

import type { FileRow, StageFiles, StageName } from "@/lib/bindings.gen";

/** A file picked in a browser, and for a bin, the entry of the object to show first. */
export interface Picked {
  row: FileRow;
  /** The object's entry hash, eight hex digits. */
  entry: string | null;
}

/** The read of a run's files at one stage. */
export interface FilesQuery {
  queryKey: QueryKey;
  queryFn: () => Promise<StageFiles | null>;
}

/** What a page that browses a run's files offers a finding's detail: a jump to a value's bin. */
export interface FileJump {
  /** The run's files at `build`, the package's, where a value's bin is found. */
  build: FilesQuery;
  /** Opens the files at `build` at a file, the object of `entry` shown first. */
  open: (row: FileRow, entry: string) => void;
}

/** The jump of the page around, when it browses the files of the run whose findings it shows. */
export const FileJumpContext = createContext<FileJump | null>(null);

/**
 * The file of the package at `build` a place names, by its WAD's name and its path. A WAD of a
 * layer other than the base one is named under the layer (`hud/Ahri.wad.client`). None for a
 * place in no file of the package, such as a game bin a patch changes.
 */
export function placeRow(files: StageFiles, wad: string, path: string): FileRow | null {
  const lower = path.toLowerCase();
  const wads = files.wads.filter((w) => w.name === wad || w.name.endsWith(`/${wad}`));
  for (const listed of wads) {
    const row = listed.files.find((file) => file.path?.toLowerCase() === lower);
    if (row && row.mark !== "removed") {
      return row;
    }
  }
  return null;
}

/** Where a run's files browser is: the stage shown, and the file picked. */
export interface FilesAt {
  /** The stage. Null for the last the run holds files at. */
  stage: StageName | null;
  picked: Picked | null;
}

/**
 * A run's files browser's place, kept by the page that shows it, and the jump that moves it to
 * a value's bin and shows the page's files tab. No jump without the run's files at `build`.
 */
export function useFilesAt(build: FilesQuery | null, showFiles: () => void) {
  const [at, setAt] = useState<FilesAt>({ stage: null, picked: null });
  const setStage = useCallback((stage: StageName) => setAt({ stage, picked: null }), []);
  const setPicked = useCallback(
    (picked: Picked | null) => setAt((current) => ({ ...current, picked })),
    [],
  );
  const jump: FileJump | null = build && {
    build,
    open: (row, entry) => {
      setAt({ stage: "build", picked: { row, entry } });
      showFiles();
    },
  };
  return { at, setStage, setPicked, jump };
}
