import { DownloadSimpleIcon } from "@phosphor-icons/react";

import { Button, FailureNote } from "@/components";
import { m } from "@/i18n";

import { useSchemaCheck, useSchemaUpdating, useUpdateSchema } from "../api/schema";
import { Offer } from "./InputRow";

/**
 * The schema database downloaded or updated by the app (#ADR-0059): whether the LTK Meta Wiki
 * publishes another than the one the League Toolkit programs share, and a download of it into
 * their folder. Nothing when the shared one is the one published.
 */
export function SchemaDownload() {
  const check = useSchemaCheck();
  const update = useUpdateSchema();
  const updating = useSchemaUpdating();

  if (check.isError) {
    return <FailureNote failure={check.error} className="text-xs" />;
  }
  if (!check.data) {
    return null;
  }
  const { held, behind } = check.data;
  if (!behind && !update.isError) {
    return null;
  }

  return (
    <Offer
      text={held ? m.inputs_schema_newer_label() : m.inputs_schema_offer_label()}
      action={
        <Button
          size="sm"
          variant="filled"
          loading={updating}
          left={<DownloadSimpleIcon weight="bold" />}
          onClick={() => update.mutate()}
        >
          {held ? m.inputs_schema_update_action() : m.inputs_schema_download_action()}
        </Button>
      }
    >
      {update.isError && <FailureNote failure={update.error} className="text-xs" />}
    </Offer>
  );
}
