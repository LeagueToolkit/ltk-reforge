import type { StageName } from "@/lib/bindings.gen";
import { FilesBrowser, type Picked } from "@/modules/files";

import { runFileQuery, useRunFiles } from "../api/runs";

interface FilesTabProps {
  id: number;
  /** The stages the run holds files for, in order. */
  stages: StageName[];
  stage: StageName;
  onStage: (stage: StageName) => void;
  /** The file picked, which a jump to a value's bin picks. */
  picked: Picked | null;
  onPick: (picked: Picked | null) => void;
}

/**
 * The files of a kept run at each stage it holds files for, each file marked against the
 * stage before.
 */
export function FilesTab({ id, stages, stage, onStage, picked, onPick }: FilesTabProps) {
  const files = useRunFiles(id, stage);
  return (
    <FilesBrowser
      files={files}
      stages={stages}
      stage={stage}
      onStage={onStage}
      fileQuery={(row, entry) => runFileQuery(id, row.content, entry)}
      picked={picked}
      onPick={onPick}
    />
  );
}
