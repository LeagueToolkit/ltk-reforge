import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { HistoryView } from "@/lib/bindings.gen";

import {
  cancelIndex,
  useIndexJobRunning,
  useIndexProgress,
  useIndexRelease,
  useInstallIndex,
} from "../api/index";
import { Offer } from "./InputRow";
import { megabytes, StepProgress } from "./ProgressRow";

interface IndexDownloadProps {
  /** The index as opened. Undefined while the inputs open. */
  installed: HistoryView | undefined;
  /** Whether another offer beside this one is the one to take first. */
  quiet?: boolean;
}

/**
 * The prebuilt history index (#ADR-0047): a download of the newest release's when the index
 * read is not that release's. An index taken further than the release is offered none, and so
 * is one that is the release's.
 */
export function IndexDownload({ installed, quiet = false }: IndexDownloadProps) {
  const release = useIndexRelease();
  const install = useInstallIndex();
  const progress = useIndexProgress();
  const running = useIndexJobRunning("install");
  const busy = useIndexJobRunning();

  if (release.isError) {
    return <FailureNote failure={release.error} className="text-xs" />;
  }
  if (!release.data) {
    return null;
  }
  const offered = release.data;
  const further = installed?.builds != null && installed.builds > offered.builds;
  const current =
    installed?.newest_build === offered.newest_build && installed.builds === offered.builds;
  if ((further || current) && !running && !install.isError) {
    return null;
  }
  const has = installed?.builds != null;
  const facts = {
    tag: offered.tag,
    build: offered.newest_build,
    builds: offered.builds,
    size: megabytes(offered.size),
  };

  return (
    <Offer
      text={has ? m.inputs_index_replace_label(facts) : m.inputs_index_offer_label(facts)}
      action={
        !running && (
          <Button
            size="sm"
            variant={has || quiet ? "outline" : "filled"}
            left={<DownloadSimpleIcon weight="bold" />}
            disabled={busy}
            onClick={() => install.mutate()}
          >
            {m.inputs_index_download_action()}
          </Button>
        )
      }
    >
      {running && progress && <StepProgress progress={progress} onStop={cancelIndex} />}
      {install.isError && <FailureNote failure={install.error} className="text-xs" />}
    </Offer>
  );
}
