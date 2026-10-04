import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { HistoryView } from "@/lib/bindings.gen";

import { cancelIndex, useIndexProgress, useIndexRelease, useInstallIndex } from "../api/index";

function megabytes(bytes: number): string {
  return (bytes / 1_000_000).toFixed(0);
}

/**
 * The prebuilt history index (#ADR-0047): the newest release's build, and a download when the
 * index read is not that release's.
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
        <div className="flex items-center gap-3">
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-surface-700">
            <div
              className="h-full bg-accent-500 transition-[width]"
              style={{ width: `${(progress.done * 100) / Math.max(progress.total, 1)}%` }}
            />
          </div>
          <span className="font-mono text-xs text-surface-300">
            {m.inputs_index_progress_label({
              done: megabytes(progress.done),
              total: megabytes(progress.total),
            })}
          </span>
          <Button size="xs" variant="ghost" onClick={cancelIndex}>
            {m.inputs_index_cancel_action()}
          </Button>
        </div>
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
