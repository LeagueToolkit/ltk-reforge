import { type ReactNode, useState } from "react";

import { Button, FailureNote, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { HistoryView, StorageView } from "@/lib/bindings.gen";

import { useIndexBehind, useIndexJobRunning, useRemoveIndex } from "../api/index";
import { useStorage } from "../api/storage";
import { ConfirmRemove } from "./ConfirmRemove";
import { IndexDownload } from "./IndexDownload";
import { IndexUpdate } from "./IndexUpdate";
import { InputRow, Offer, RemoveButton, StatusBadge } from "./InputRow";
import { diskSize } from "./ProgressRow";

/** Where the index the settings choose came from, in words. */
function originOf(dir: string, storage: StorageView | undefined): string | null {
  if (!storage) {
    return null;
  }
  if (!storage.index_owned) {
    return m.inputs_index_origin_chosen_label();
  }
  const folder = dir.split(/[\\/]/).pop() ?? "";
  // An update names its folder for the commit it reached, a download for its release.
  return folder.startsWith("head-")
    ? m.inputs_index_origin_updated_label()
    : m.inputs_index_origin_release_label({ tag: folder });
}

function Badge({ chosen, opened }: { chosen: boolean; opened: boolean }) {
  const behind = useIndexBehind();
  if (!chosen) {
    return <StatusBadge tone="muted">{m.inputs_status_none_label()}</StatusBadge>;
  }
  if (!opened) {
    return <StatusBadge tone="bad">{m.inputs_status_unreadable_label()}</StatusBadge>;
  }
  if (behind.isFetching) {
    return <StatusBadge tone="busy">{m.inputs_status_checking_label()}</StatusBadge>;
  }
  if (behind.data === true) {
    return <StatusBadge tone="warn">{m.inputs_index_status_behind_label()}</StatusBadge>;
  }
  if (behind.data === false) {
    return <StatusBadge tone="ok">{m.inputs_status_current_label()}</StatusBadge>;
  }
  return null;
}

interface IndexRowProps {
  /** The index as opened. Undefined while the inputs open. */
  view: HistoryView | undefined;
  /** The folder the settings choose, or null. */
  chosen: string | null;
  /** Makes a folder the index the runs read, or none. */
  onChoose: (dir: string | null) => void;
  /** Asks the user for a folder. */
  pick: ReactNode;
}

/**
 * The history index: the database a run reads. Its state against the census history, where it
 * came from, the two ways it is brought up to date (the builds added here, or the newest
 * release downloaded), and its removal from disk (#ADR-0061).
 */
export function IndexRow({ view, chosen, onChoose, pick }: IndexRowProps) {
  const storage = useStorage();
  const remove = useRemoveIndex();
  const busy = useIndexJobRunning();
  const [asking, setAsking] = useState(false);

  const kept = storage.data?.kept;
  const removable = storage.data != null && (storage.data.index_owned || kept != null);
  const freed = (storage.data?.index_owned ? storage.data.index_bytes : 0) + (kept?.bytes ?? 0);

  return (
    <InputRow
      title={m.inputs_history_title()}
      kind={m.inputs_history_kind_label()}
      badge={view && <Badge chosen={chosen != null} opened={view.dir != null} />}
      description={m.inputs_history_description()}
      path={chosen}
      place="index"
      none={m.inputs_history_none()}
      facts={
        chosen && (
          <>
            {view?.builds != null && view.newest_build && (
              <span>
                {m.inputs_history_builds_label({ builds: view.builds, build: view.newest_build })}
              </span>
            )}
            {originOf(chosen, storage.data) && <span>{originOf(chosen, storage.data)}</span>}
            {storage.data && storage.data.index_bytes > 0 && (
              <span>{m.inputs_on_disk_label({ size: diskSize(storage.data.index_bytes) })}</span>
            )}
          </>
        )
      }
      actions={
        <>
          {removable && (
            <RemoveButton
              label={m.inputs_remove_action()}
              disabled={busy}
              onClick={() => setAsking(true)}
            />
          )}
          {chosen && (
            <Tooltip content={m.inputs_history_use_none_hint()}>
              <Button size="sm" variant="ghost" disabled={busy} onClick={() => onChoose(null)}>
                {m.inputs_history_use_none_action()}
              </Button>
            </Tooltip>
          )}
          {pick}
        </>
      }
    >
      {chosen && view?.dir && <IndexUpdate />}
      {kept && !chosen && (
        <Offer
          text={m.inputs_index_kept_label({
            builds: kept.builds,
            build: kept.newest_build,
            size: diskSize(kept.bytes),
          })}
          action={
            <Button size="sm" variant="filled" disabled={busy} onClick={() => onChoose(kept.dir)}>
              {m.inputs_index_use_kept_action()}
            </Button>
          }
        />
      )}
      <IndexDownload installed={view} quiet={kept != null && !chosen} />
      {remove.isError && <FailureNote failure={remove.error} className="text-xs" />}
      <ConfirmRemove
        open={asking}
        onClose={() => setAsking(false)}
        title={m.inputs_index_remove_title()}
        description={m.inputs_index_remove_description({ size: diskSize(freed) })}
        onConfirm={() => remove.mutate()}
      />
    </InputRow>
  );
}
