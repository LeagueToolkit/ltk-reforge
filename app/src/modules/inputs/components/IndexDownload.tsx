import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { HistoryView } from "@/lib/bindings.gen";

import { cancelIndex, useIndexProgress, useIndexRelease, useInstallIndex } from "../api/index";
import { megabytes, ProgressRow } from "./ProgressRow";

/**
 * The prebuilt history index (#ADR-0047): the newest release's build, and a download when the
 * index read is not that release's. An index taken further than the release is offered none.
 */
export function IndexDownload({ installed }: { installed: HistoryView | undefined }) {
  const release = useIndexRelease();
  const install = useInstallIndex();
  const progress = useIndexProgress();

  if (release.isError) {
    return <FailureNote failure={release.error} className="text-xs" />;
  }
  if (!release.data) {
    return null;
  }
  const offered = release.data;
  if (installed?.builds != null && installed.builds > offered.builds) {
    return null;
  }
  const current =
    installed?.newest_build === offered.newest_build && installed.builds === offered.builds;

  return (
    <div className="mt-2 flex flex-col gap-2" data-ui="IndexDownload">
      <p className="text-xs text-surface-400 select-none">
        {current
          ? m.inputs_index_current_label({ tag: offered.tag })
          : m.inputs_index_offer_label({
              tag: offered.tag,
              build: offered.newest_build,
              builds: offered.builds,
              size: megabytes(offered.size),
            })}
      </p>
      {install.isPending && progress && (
        <ProgressRow
          fraction={progress.done / Math.max(progress.total, 1)}
          label={m.inputs_download_progress_label({
            done: megabytes(progress.done),
            total: megabytes(progress.total),
          })}
          onStop={cancelIndex}
        />
      )}
      {!current && !install.isPending && (
        <div>
          <Button
            size="sm"
            variant="filled"
            left={<DownloadSimpleIcon weight="bold" />}
            onClick={() => install.mutate()}
          >
            {m.inputs_index_download_action()}
          </Button>
        </div>
      )}
      {install.isError && <FailureNote failure={install.error} className="text-xs" />}
    </div>
  );
}
