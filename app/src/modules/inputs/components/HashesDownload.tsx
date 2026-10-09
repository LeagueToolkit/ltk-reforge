import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";

import {
  cancelHashes,
  useHashesCheck,
  useHashesProgress,
  useHashesUpdating,
  useUpdateHashes,
} from "../api/hashes";
import { Offer } from "./InputRow";
import { megabytes, ProgressRow } from "./ProgressRow";

/**
 * The hash tables downloaded or updated by the app (#ADR-0059): the tables the newest release
 * has that the cache lacks or holds older, and a download of them into the cache every League
 * Toolkit program reads. Nothing when the cache holds them all.
 */
export function HashesDownload({ installed }: { installed: boolean }) {
  const check = useHashesCheck();
  const update = useUpdateHashes();
  const updating = useHashesUpdating();
  const progress = useHashesProgress();

  if (check.isError) {
    return <FailureNote failure={check.error} className="text-xs" />;
  }
  if (!check.data) {
    return null;
  }
  const { behind, bytes } = check.data;
  if (behind.length === 0 && !updating && !update.isError) {
    return null;
  }
  const tables = behind.join(", ");

  return (
    <Offer
      text={
        bytes != null
          ? m.inputs_hashes_offer_label({ tables, size: megabytes(bytes) })
          : m.inputs_hashes_offer_unsized_label({ tables })
      }
      action={
        !updating && (
          <Button
            size="sm"
            variant="filled"
            left={<DownloadSimpleIcon weight="bold" />}
            onClick={() => update.mutate()}
          >
            {installed ? m.inputs_hashes_update_action() : m.inputs_hashes_download_action()}
          </Button>
        )
      }
    >
      {updating && progress && (
        <ProgressRow
          fraction={progress.total > 0 ? progress.done / progress.total : null}
          label={m.inputs_download_progress_label({
            done: megabytes(progress.done),
            total: megabytes(progress.total),
          })}
          onStop={cancelHashes}
        />
      )}
      {update.isError && <FailureNote failure={update.error} className="text-xs" />}
    </Offer>
  );
}
