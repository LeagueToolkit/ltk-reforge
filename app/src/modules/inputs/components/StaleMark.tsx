import { WarningIcon } from "@phosphor-icons/react";
import { Link } from "@tanstack/react-router";

import { Tooltip } from "@/components";
import { m } from "@/i18n";

import { type StaleInputs, useStaleInputs } from "../api/stale";

/** What a download would bring up to date, input by input, in words. Null for one that is current. */
export function staleWords(stale: StaleInputs) {
  const schema = stale.schema?.held ? m.inputs_schema_stale_text() : m.inputs_schema_absent_text();
  return {
    hashes:
      stale.hashes.length > 0
        ? m.inputs_hashes_stale_text({ tables: stale.hashes.join(", ") })
        : null,
    schema: stale.schema ? schema : null,
    history: stale.history ? m.inputs_index_stale_text() : null,
  };
}

/** Every stale input in words, in the order of the inputs page. */
function useStaleLines(): string[] {
  const words = staleWords(useStaleInputs());
  return [words.hashes, words.schema, words.history].filter((line) => line != null);
}

function Lines({ lines }: { lines: string[] }) {
  return (
    <ul className="flex flex-col gap-0.5">
      {lines.map((line) => (
        <li key={line}>{line}</li>
      ))}
    </ul>
  );
}

/** A warning triangle that says, while hovered, what is stale. Nothing when nothing is. */
export function StaleMark({ lines }: { lines: string[] }) {
  if (lines.length === 0) {
    return null;
  }
  return (
    <Tooltip content={<Lines lines={lines} />}>
      <span
        role="img"
        aria-label={lines.join(". ")}
        className="inline-flex shrink-0 text-warning-text"
        data-ui="StaleMark"
      >
        <WarningIcon weight="bold" className="h-4 w-4" />
      </span>
    </Tooltip>
  );
}

/** The mark of every stale input, for beside the inputs page's name. */
export function StaleInputsMark() {
  return <StaleMark lines={useStaleLines()} />;
}

/** A link to the inputs page while an input is stale, which says, while hovered, which. */
export function StaleInputsLink() {
  const lines = useStaleLines();
  if (lines.length === 0) {
    return null;
  }
  return (
    <Tooltip content={<Lines lines={lines} />} side="bottom" align="start">
      <Link
        to="/inputs"
        className="flex items-center gap-1.5 rounded-md bg-warning/10 px-2.5 py-1 text-sm text-warning-text select-none hover:bg-warning/20"
        data-ui="StaleInputsLink"
      >
        <WarningIcon weight="bold" className="h-4 w-4" />
        {m.inputs_stale_action()}
      </Link>
    </Tooltip>
  );
}
