import { Outlet } from "@tanstack/react-router";

import { FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";

import { useRecords } from "../api/records";
import { RecordList } from "./RecordList";

/**
 * The records under `docs/`, bundled into the app (#ADR-0091): the list on the left and the
 * record of the route on the right, as the Runs page. The records are read-only here and
 * edited in the repository.
 */
export function RecordsPage() {
  const records = useRecords();
  return (
    <div className="flex flex-col gap-6" data-ui="RecordsPage">
      <h1 className="font-display text-xl font-semibold text-surface-50">
        {m.records_page_title()}
      </h1>
      {records.isPending && <Spinner />}
      {records.isError && <FailureNote failure={records.error} />}
      {records.isSuccess && (
        <div className="flex items-start gap-6">
          <RecordList
            heads={records.data}
            // The window less the shell's padding and the page's title above the list.
            className="sticky top-0 max-h-[calc(100vh-8rem)] w-1/3 min-w-56 shrink-0"
          />
          <div className="min-w-0 flex-1">
            <Outlet />
          </div>
        </div>
      )}
    </div>
  );
}

/** The record pane with no record picked. */
export function RecordsEmpty() {
  return <p className="text-sm text-surface-400 select-none">{m.records_pick_hint()}</p>;
}
