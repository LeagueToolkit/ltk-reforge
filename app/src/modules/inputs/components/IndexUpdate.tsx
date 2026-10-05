import { ArrowsClockwiseIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";
import type { UpdateStep } from "@/lib/bindings.gen";

import { cancelIndex, useIndexBehind, useIndexUpdateStep, useUpdateIndex } from "../api/index";
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
 * The index taken forward from the census history (#ADR-0057): whether the history holds builds
 * after the index's head, and an update that fetches them and appends them to a copy of the
 * index.
 */
export function IndexUpdate() {
  const behind = useIndexBehind();
  const update = useUpdateIndex();
  const step = useIndexUpdateStep();

  if (behind.isError) {
    return <FailureNote failure={behind.error} className="text-xs" />;
  }
  if (behind.data == null) {
    return null;
  }

  return (
    <div className="mt-2 flex flex-col gap-2" data-ui="IndexUpdate">
      <p className="text-xs text-surface-400 select-none">
        {behind.data ? m.inputs_index_behind_label() : m.inputs_index_up_to_date_label()}
      </p>
      {update.isPending && step && <Step step={step} />}
      {behind.data && !update.isPending && (
        <div>
          <Button
            size="sm"
            variant="filled"
            left={<ArrowsClockwiseIcon weight="bold" />}
            onClick={() => update.mutate()}
          >
            {m.inputs_index_update_action()}
          </Button>
        </div>
      )}
      {update.isError && <FailureNote failure={update.error} className="text-xs" />}
    </div>
  );
}
