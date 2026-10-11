import { StopIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";

import { useStopRun } from "../api/run";
import { useRepairStore } from "../state/repairStore";

/** Stop, in the Run button's place while a run is under way (#ADR-0097). */
export function StopButton() {
  const stop = useStopRun();
  const stopping = useRepairStore((state) => state.live?.stopping ?? false);
  return (
    <>
      <Button
        variant="outline"
        left={<StopIcon weight="bold" />}
        loading={stopping}
        onClick={() => stop.mutate()}
      >
        {m.run_stop_action()}
      </Button>
      {stop.isError && <FailureNote failure={stop.error} />}
    </>
  );
}
