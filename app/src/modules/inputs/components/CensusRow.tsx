import { DownloadSimpleIcon, FolderOpenIcon } from "@phosphor-icons/react";
import { useState } from "react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { CloneView } from "@/lib/bindings.gen";

import {
  cancelIndex,
  useArchiveProgress,
  useBootstrapCensus,
  useCensusClone,
  useIndexJobRunning,
  useIndexRelease,
  useRemoveCensus,
  useRemoveDownloads,
} from "../api/index";
import { useReveal, useStorage } from "../api/storage";
import { ConfirmRemove } from "./ConfirmRemove";
import { InputRow, Offer, RemoveButton, StatusBadge } from "./InputRow";
import { diskSize, megabytes, StepProgress } from "./ProgressRow";

function Badge({ clone }: { clone: CloneView }) {
  switch (clone.kind) {
    case "whole":
      return <StatusBadge tone="ok">{m.inputs_census_status_whole_label()}</StatusBadge>;
    case "partial":
      return <StatusBadge tone="muted">{m.inputs_census_status_partial_label()}</StatusBadge>;
    case "none":
      return <StatusBadge tone="muted">{m.inputs_status_none_label()}</StatusBadge>;
  }
}

function stateOf(clone: CloneView): string {
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
 * The census history on this machine (#ADR-0060): the app's clone of the repository the index
 * is built from, which no run reads. How much of it is here, a download of the newest
 * release's archive that makes it whole, and its removal from disk (#ADR-0061).
 */
export function CensusRow() {
  const clone = useCensusClone();
  const release = useIndexRelease();
  const storage = useStorage();
  const bootstrap = useBootstrapCensus();
  const remove = useRemoveCensus();
  const reveal = useReveal();
  const progress = useArchiveProgress();
  const running = useIndexJobRunning("bootstrap");
  const busy = useIndexJobRunning();
  const [asking, setAsking] = useState(false);

  if (!clone.data) {
    return null;
  }
  const held = clone.data.kind !== "none";
  const bytes = storage.data?.census_bytes ?? 0;
  const archive = release.data?.archive_size;

  return (
    <InputRow
      title={m.inputs_census_title()}
      kind={m.inputs_census_kind_label()}
      badge={<Badge clone={clone.data} />}
      description={m.inputs_census_description()}
      facts={
        <>
          <span>{stateOf(clone.data)}</span>
          {bytes > 0 && <span>{m.inputs_on_disk_label({ size: diskSize(bytes) })}</span>}
        </>
      }
      actions={
        <>
          {bytes > 0 && (
            <RemoveButton
              label={m.inputs_remove_action()}
              disabled={busy}
              onClick={() => setAsking(true)}
            />
          )}
          {held && (
            <Button
              size="sm"
              variant="outline"
              left={<FolderOpenIcon weight="bold" />}
              onClick={() => reveal.mutate("census")}
            >
              {m.inputs_reveal_action()}
            </Button>
          )}
        </>
      }
    >
      {(archive != null || running) && (
        <Offer
          text={
            clone.data.kind === "whole"
              ? m.inputs_census_again_label()
              : m.inputs_census_download_label()
          }
          action={
            !running &&
            archive != null && (
              <Button
                size="sm"
                variant="outline"
                left={<DownloadSimpleIcon weight="bold" />}
                disabled={busy}
                onClick={() => bootstrap.mutate()}
              >
                {clone.data.kind === "whole"
                  ? m.inputs_census_again_action({ size: megabytes(archive) })
                  : m.inputs_census_download_action({ size: megabytes(archive) })}
              </Button>
            )
          }
        >
          {running && progress && <StepProgress progress={progress} onStop={cancelIndex} />}
          {bootstrap.isError && <FailureNote failure={bootstrap.error} className="text-xs" />}
        </Offer>
      )}
      {reveal.isError && <FailureNote failure={reveal.error} className="text-xs" />}
      {remove.isError && <FailureNote failure={remove.error} className="text-xs" />}
      <ConfirmRemove
        open={asking}
        onClose={() => setAsking(false)}
        title={m.inputs_census_remove_title()}
        description={m.inputs_census_remove_description({ size: diskSize(bytes) })}
        onConfirm={() => remove.mutate()}
      />
    </InputRow>
  );
}

/**
 * The downloads a stop kept (#ADR-0060): how much they hold, and their removal (#ADR-0061).
 * Nothing when none is kept.
 */
export function KeptDownloads() {
  const storage = useStorage();
  const remove = useRemoveDownloads();
  const busy = useIndexJobRunning();
  const bytes = storage.data?.downloads_bytes ?? 0;

  if (bytes === 0 && !remove.isError) {
    return null;
  }
  return (
    <InputRow
      title={m.inputs_downloads_title()}
      description={m.inputs_downloads_description()}
      facts={bytes > 0 && <span>{m.inputs_on_disk_label({ size: diskSize(bytes) })}</span>}
      actions={
        bytes > 0 && (
          <RemoveButton
            label={m.inputs_remove_action()}
            disabled={busy}
            onClick={() => remove.mutate()}
          />
        )
      }
    >
      {remove.isError && <FailureNote failure={remove.error} className="text-xs" />}
    </InputRow>
  );
}
