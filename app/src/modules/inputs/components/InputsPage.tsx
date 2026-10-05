import { ArrowsClockwiseIcon } from "@phosphor-icons/react";

import { Button, FailureNote, SectionCard, Spinner } from "@/components";
import { m } from "@/i18n";
import type { InputsView, Settings } from "@/lib/bindings.gen";
import { FindingList } from "@/modules/findings";
import { useSaveSettings, useSettings } from "@/modules/settings";

import { useHashesCheck } from "../api/hashes";
import { useFindGame, useInputs } from "../api/inputs";
import { pickFile, pickFolder } from "../api/pick";
import { useSchemaCheck } from "../api/schema";
import { useCheckInputs } from "../api/stale";
import { CensusRow, KeptDownloads } from "./CensusRow";
import { HashesDownload } from "./HashesDownload";
import { IndexRow } from "./IndexRow";
import { InputRow, StatusBadge } from "./InputRow";
import { SchemaDownload } from "./SchemaDownload";

type InputKey = "inputs.game" | "inputs.hashes" | "inputs.schema" | "inputs.history";

function useSetInput() {
  const settings = useSettings();
  const save = useSaveSettings();
  return (key: InputKey, value: string | null) => {
    if (settings.data) {
      const next: Settings = { ...settings.data, [key]: value };
      save.mutate(next);
    }
  };
}

function HashesBadge({ installed }: { installed: boolean }) {
  const check = useHashesCheck();
  if (check.isFetching) {
    return <StatusBadge tone="busy">{m.inputs_status_checking_label()}</StatusBadge>;
  }
  if (!check.data) {
    return installed ? null : (
      <StatusBadge tone="muted">{m.inputs_status_none_label()}</StatusBadge>
    );
  }
  if (check.data.behind.length === 0) {
    return <StatusBadge tone="ok">{m.inputs_status_current_label()}</StatusBadge>;
  }
  return installed ? (
    <StatusBadge tone="warn">{m.inputs_status_update_label()}</StatusBadge>
  ) : (
    <StatusBadge tone="muted">{m.inputs_status_none_label()}</StatusBadge>
  );
}

function SchemaBadge({ own, read }: { own: boolean; read: boolean }) {
  const check = useSchemaCheck();
  if (own) {
    // A file the user chose is checked against nothing.
    return read ? null : <StatusBadge tone="bad">{m.inputs_status_unreadable_label()}</StatusBadge>;
  }
  if (check.isFetching) {
    return <StatusBadge tone="busy">{m.inputs_status_checking_label()}</StatusBadge>;
  }
  if (!check.data) {
    return read ? null : <StatusBadge tone="muted">{m.inputs_status_none_label()}</StatusBadge>;
  }
  if (!check.data.behind) {
    return <StatusBadge tone="ok">{m.inputs_status_current_label()}</StatusBadge>;
  }
  return check.data.held ? (
    <StatusBadge tone="warn">{m.inputs_status_update_label()}</StatusBadge>
  ) : (
    <StatusBadge tone="muted">{m.inputs_status_none_label()}</StatusBadge>
  );
}

function Rows({ view }: { view: InputsView | undefined }) {
  const settings = useSettings();
  const set = useSetInput();
  const find = useFindGame();
  const chosen = settings.data ?? {};

  const choose = async (key: InputKey) => {
    const picked = await pickFolder();
    if (picked) {
      set(key, picked);
    }
  };
  const chooseSchema = async () => {
    const picked = await pickFile(["json"], m.inputs_schema_title());
    if (picked) {
      set("inputs.schema", picked);
    }
  };
  const findGame = () =>
    find.mutate(undefined, {
      onSuccess: (game) => {
        if (game) {
          set("inputs.game", game);
        }
      },
    });
  const game = view?.game.path ?? chosen["inputs.game"];

  return (
    <div className="flex flex-col divide-y divide-surface-700/50">
      <InputRow
        title={m.inputs_game_title()}
        badge={!game && <StatusBadge tone="bad">{m.inputs_status_not_chosen_label()}</StatusBadge>}
        description={m.inputs_game_description()}
        path={game}
        place="game"
        none={m.inputs_game_none()}
        facts={
          <>
            {view?.game.build && (
              <span>{m.inputs_game_build_label({ build: view.game.build })}</span>
            )}
            {view && view.game.unreadable_wads > 0 && (
              <span className="text-warning-text">
                {m.inputs_game_unreadable_wads_label({ count: view.game.unreadable_wads })}
              </span>
            )}
            {find.isSuccess && !find.data && (
              <span className="text-warning-text">{m.inputs_find_none_hint()}</span>
            )}
          </>
        }
        actions={
          <>
            <Button size="sm" variant="ghost" loading={find.isPending} onClick={findGame}>
              {m.inputs_find_action()}
            </Button>
            <Button size="sm" variant="outline" onClick={() => void choose("inputs.game")}>
              {m.inputs_choose_folder_action()}
            </Button>
          </>
        }
      />
      <InputRow
        title={m.inputs_hashes_title()}
        badge={view && <HashesBadge installed={view.hashes.tables.length > 0} />}
        description={m.inputs_hashes_description()}
        path={view?.hashes.dir}
        place="hashes"
        none={m.inputs_hashes_none()}
        facts={
          view?.hashes.dir && (
            <>
              <span>
                {chosen["inputs.hashes"] ? m.inputs_own_folder_label() : m.inputs_shared_label()}
              </span>
              {view.hashes.tables.length > 0 && (
                <span>
                  {m.inputs_hashes_tables_label({ tables: view.hashes.tables.join(", ") })}
                </span>
              )}
            </>
          )
        }
        actions={
          <>
            {chosen["inputs.hashes"] && (
              <Button size="sm" variant="ghost" onClick={() => set("inputs.hashes", null)}>
                {m.inputs_use_shared_action()}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={() => void choose("inputs.hashes")}>
              {m.inputs_choose_folder_action()}
            </Button>
          </>
        }
      >
        {view && <HashesDownload installed={view.hashes.tables.length > 0} />}
      </InputRow>
      <InputRow
        title={m.inputs_schema_title()}
        badge={
          view && (
            <SchemaBadge own={chosen["inputs.schema"] != null} read={view.schema.path != null} />
          )
        }
        description={m.inputs_schema_description()}
        path={view?.schema.path}
        place="schema"
        none={m.inputs_schema_none()}
        facts={
          view?.schema.path && (
            <>
              <span>
                {chosen["inputs.schema"] ? m.inputs_own_file_label() : m.inputs_shared_label()}
              </span>
              {view.schema.latest_build && (
                <span>{m.inputs_schema_build_label({ build: view.schema.latest_build })}</span>
              )}
            </>
          )
        }
        actions={
          <>
            {chosen["inputs.schema"] && (
              <Button size="sm" variant="ghost" onClick={() => set("inputs.schema", null)}>
                {m.inputs_use_shared_action()}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={() => void chooseSchema()}>
              {m.inputs_choose_file_action()}
            </Button>
          </>
        }
      >
        {view && !chosen["inputs.schema"] && <SchemaDownload />}
      </InputRow>
      <IndexRow
        view={view?.history}
        chosen={chosen["inputs.history"] ?? null}
        onChoose={(dir) => set("inputs.history", dir)}
        pick={
          <Button size="sm" variant="outline" onClick={() => void choose("inputs.history")}>
            {m.inputs_choose_folder_action()}
          </Button>
        }
      />
    </div>
  );
}

/**
 * The inputs a run is judged against: each one, where it was read from, how it stands against
 * what is published, and what was found. Under them, what the history index is made from, which
 * no run reads.
 */
export function InputsPage() {
  const inputs = useInputs();
  const { check, checking } = useCheckInputs();

  return (
    <div className="flex max-w-3xl flex-col gap-6" data-ui="InputsPage">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="font-display text-xl font-semibold text-surface-50">
            {m.inputs_page_title()}
          </h1>
          <p className="text-sm text-surface-400">{m.inputs_page_description()}</p>
        </div>
        <Button
          size="sm"
          variant="outline"
          loading={checking}
          left={<ArrowsClockwiseIcon weight="bold" />}
          onClick={check}
        >
          {m.inputs_check_action()}
        </Button>
      </div>
      <SectionCard title={m.inputs_read_title()} description={m.inputs_read_description()}>
        <Rows view={inputs.data} />
        {inputs.isFetching && (
          <p className="flex items-center gap-2 text-xs text-surface-400 select-none">
            <Spinner size="sm" />
            {m.inputs_opening_label()}
          </p>
        )}
        {inputs.isError && <FailureNote failure={inputs.error} />}
      </SectionCard>
      <SectionCard title={m.inputs_source_title()} description={m.inputs_source_description()}>
        <div className="flex flex-col divide-y divide-surface-700/50">
          <CensusRow />
          <KeptDownloads />
        </div>
      </SectionCard>
      {inputs.data && (
        <SectionCard title={m.inputs_findings_title()}>
          <FindingList findings={inputs.data.findings} empty={m.inputs_findings_empty()} />
        </SectionCard>
      )}
    </div>
  );
}
