import type { StageName } from "@/lib/bindings.gen";
import { FilesBrowser } from "@/modules/files";

import { runFileQuery, useRunFiles } from "../api/runs";

interface FilesTabProps {
  id: number;
  /** The stages the run holds files for, in order. */
  stages: StageName[];
  stage: StageName;
  onStage: (stage: StageName) => void;
}

/**
 * The files of a kept run at each stage it holds files for, each file marked against the
 * stage before.
 */
export function FilesTab({ id, stages, stage, onStage }: FilesTabProps) {
  const files = useRunFiles(id, stage);
  return (
    <FilesBrowser
      files={files}
      stages={stages}
      stage={stage}
      onStage={onStage}
      fileQuery={(row) => runFileQuery(id, row.content)}
    />
  );
}
