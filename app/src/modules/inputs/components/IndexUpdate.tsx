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
import { kilobytes, ProgressRow } from "./ProgressRow";

function Step({ step }: { step: UpdateStep }) {
  if (step.kind === "fetch") {
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
  }
  return (
    <ProgressRow
      fraction={step.done / Math.max(step.of, 1)}
      label={m.inputs_index_append_label({ done: step.done, of: step.of, build: step.build })}
    />
  );
}

/**
 * The index taken forward from the census history (#ADR-0057): an update that fetches the
 * builds after the index's head and appends them to a copy of the index. Nothing when the
 * index holds every build.
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
