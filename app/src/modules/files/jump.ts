import type { QueryKey } from "@tanstack/react-query";
import { createContext, useCallback, useRef, useState } from "react";

import type { FileRow, FileView, RunRef, StageFiles, StageName } from "@/lib/bindings.gen";

/** A place in a bin a jump asks for: an object, and a value in it. */
export interface BinPlace {
  /** The object's entry hash, eight hex digits. */
  entry: string;
  /** The value's path in the object in the hash form, empty for the object itself. */
  at: string;
}

/** A file picked in a browser, and for a bin, the place a jump asked for in it. */
export interface Picked {
  row: FileRow;
  /** The place the bin opens at. Null for a file a person picked in the tree. */
  place: BinPlace | null;
  /**
   * The number of the jump that picked it, each jump's its own, so a jump to the place shown
   * already lands again. None for a file a person picked in the tree.
   */
  jump?: number;
}

/** The key of the row a place names in a bin's tree, as the tree's rows are keyed. */
export function placeKey(place: BinPlace): string {
  return `0x${place.entry}:${place.at}`;
}

/** The read of one file's bytes as the app shows them, and the run it is one of. */
export interface FileQuery {
  queryKey: QueryKey;
  queryFn: () => Promise<FileView>;
  /** The run whose file it is, which a bin's rows are read from. */
  run: RunRef;
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
  /** Opens the files at `build` at a file, its tree opened at `place`. */
  open: (row: FileRow, place: BinPlace) => void;
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
  const jumps = useRef(0);
  const setStage = useCallback((stage: StageName) => setAt({ stage, picked: null }), []);
  const setPicked = useCallback(
    (picked: Picked | null) => setAt((current) => ({ ...current, picked })),
    [],
  );
  const jump: FileJump | null = build && {
    build,
    open: (row, place) => {
      jumps.current += 1;
      setAt({ stage: "build", picked: { row, place, jump: jumps.current } });
      showFiles();
    },
  };
  return { at, setStage, setPicked, jump };
}
