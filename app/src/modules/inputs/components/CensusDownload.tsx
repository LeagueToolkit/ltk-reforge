import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { CloneView } from "@/lib/bindings.gen";

import {
  cancelIndex,
  useArchiveProgress,
  useBootstrapCensus,
  useCensusClone,
  useIndexRelease,
} from "../api/index";
import { megabytes, StepProgress } from "./ProgressRow";

function cloneLabel(clone: CloneView): string {
  switch (clone.kind) {
    case "whole":
      return m.inputs_census_whole_label({ tag: clone.tag });
    case "partial":
      return m.inputs_census_partial_label();
    case "none":
      return m.inputs_census_none_label();
  }
}

/**
 * The census history on this machine (#ADR-0060): the app's clone, fetched as far as updates read
 * it or whole, and a download of the newest release's archive that makes it whole, or makes it
 * again.
 */
export function CensusDownload() {
  const clone = useCensusClone();
  const release = useIndexRelease();
  const bootstrap = useBootstrapCensus();
  const progress = useArchiveProgress();

  if (!clone.data) {
    return null;
  }
  const size = release.data?.archive_size;

  return (
    <div className="mt-2 flex flex-col gap-2" data-ui="CensusDownload">
      <p className="text-xs text-surface-400 select-none">{cloneLabel(clone.data)}</p>
      {bootstrap.isPending && progress && <StepProgress progress={progress} onStop={cancelIndex} />}
      {size != null && !bootstrap.isPending && (
        <div>
          <Button
            size="sm"
            variant="outline"
            left={<DownloadSimpleIcon weight="bold" />}
            onClick={() => bootstrap.mutate()}
          >
            {clone.data.kind === "whole"
              ? m.inputs_census_again_action({ size: megabytes(size) })
              : m.inputs_census_download_action({ size: megabytes(size) })}
          </Button>
        </div>
      )}
      {bootstrap.isError && <FailureNote failure={bootstrap.error} className="text-xs" />}
    </div>
  );
}
