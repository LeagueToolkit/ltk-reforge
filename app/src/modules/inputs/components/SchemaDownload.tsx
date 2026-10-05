import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";

import { useSchemaCheck, useUpdateSchema } from "../api/schema";

/**
 * The schema database downloaded or updated by the app (#ADR-0059): whether the LTK Meta Wiki
 * publishes another than the one the League Toolkit programs share, and a download of it into
 * their folder.
 */
export function SchemaDownload() {
  const check = useSchemaCheck();
  const update = useUpdateSchema();

  if (check.isError) {
    return <FailureNote failure={check.error} className="text-xs" />;
  }
  if (!check.data) {
    return null;
  }
  const { held, behind } = check.data;

  return (
    <div className="mt-2 flex flex-col gap-2" data-ui="SchemaDownload">
      <p className="text-xs text-surface-400 select-none">
        {!behind
          ? m.inputs_schema_current_label()
          : held
            ? m.inputs_schema_newer_label()
            : m.inputs_schema_offer_label()}
      </p>
      {behind && (
        <div>
          <Button
            size="sm"
            variant="filled"
            loading={update.isPending}
            left={<DownloadSimpleIcon weight="bold" />}
            onClick={() => update.mutate()}
          >
            {held ? m.inputs_schema_update_action() : m.inputs_schema_download_action()}
          </Button>
        </div>
      )}
      {update.isError && <FailureNote failure={update.error} className="text-xs" />}
    </div>
  );
}
