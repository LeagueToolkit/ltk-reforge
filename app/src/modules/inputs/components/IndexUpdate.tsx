import { ArrowsClockwiseIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { UpdateStep } from "@/lib/bindings.gen";

import {
  cancelIndex,
  useIndexBehind,
  useIndexJobRunning,
  useIndexUpdateStep,
  useUpdateIndex,
} from "../api/index";
import { Offer } from "./InputRow";
import { kilobytes, megabytes, ProgressRow } from "./ProgressRow";

function Step({ step }: { step: UpdateStep }) {
  switch (step.kind) {
    case "fetch":
      return (
        <ProgressRow
          fraction={null}
          label={m.inputs_index_fetch_label({
            objects: step.objects,
            size: kilobytes(step.bytes),
          })}
          onStop={cancelIndex}
        />
      );
    case "append":
      return (
        <ProgressRow
          fraction={step.done / Math.max(step.of, 1)}
          label={m.inputs_index_append_label({ done: step.done, of: step.of, build: step.build })}
        />
      );
    case "download":
      return (
        <ProgressRow
          fraction={step.done / Math.max(step.total, 1)}
          label={m.inputs_index_delta_download_label({
            done: megabytes(step.done),
            total: megabytes(step.total),
          })}
          onStop={cancelIndex}
        />
      );
    case "write":
      return (
        <ProgressRow
          fraction={step.done / Math.max(step.total, 1)}
          label={m.inputs_index_write_label({
            done: megabytes(step.done),
            total: megabytes(step.total),
          })}
        />
      );
    case "check":
      return <ProgressRow fraction={null} label={m.inputs_index_check_label()} />;
  }
}

/**
 * The index taken forward (#ADR-0109, #ADR-0057): the release's delta downloaded and applied
 * to the release's index, or the builds after the index's head fetched from the census history
 * and appended to a copy. Nothing when the index holds every build.
 */
export function IndexUpdate() {
  const behind = useIndexBehind();
  const update = useUpdateIndex();
  const step = useIndexUpdateStep();
  const running = useIndexJobRunning("update");
  const busy = useIndexJobRunning();

  if (behind.isError) {
    return <FailureNote failure={behind.error} className="text-xs" />;
  }
  if (!behind.data && !running && !update.isError) {
    return null;
  }

  return (
    <Offer
      text={m.inputs_index_behind_label()}
      action={
        !running && (
          <Button
            size="sm"
            variant="filled"
            left={<ArrowsClockwiseIcon weight="bold" />}
            disabled={busy}
            onClick={() => update.mutate()}
          >
            {m.inputs_index_update_action()}
          </Button>
        )
      }
    >
      {running && step && <Step step={step} />}
      {update.isError && <FailureNote failure={update.error} className="text-xs" />}
    </Offer>
  );
}
