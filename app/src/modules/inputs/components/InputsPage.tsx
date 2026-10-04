import type { ReactNode } from "react";

import { Button, FailureNote, SectionCard, Spinner } from "@/components";
import { m } from "@/i18n";
import type { InputsView, Settings } from "@/lib/bindings.gen";
import { FindingList } from "@/modules/findings";
import { useSaveSettings, useSettings } from "@/modules/settings";

import { useFindGame, useInputs } from "../api/inputs";
import { pickFile, pickFolder } from "../api/pick";
import { IndexDownload } from "./IndexDownload";

type InputKey = "inputs.game" | "inputs.hashes" | "inputs.schema" | "inputs.history";

interface InputRowProps {
  title: string;
  description: string;
  /** The path read, or null. */
  path: string | null | undefined;
  /** What the row says when no path was read. */
  none: string;
  /** Lines under the path: a build, a count. */
  facts?: ReactNode;
  actions: ReactNode;
}

function InputRow({ title, description, path, none, facts, actions }: InputRowProps) {
  return (
    <div className="flex items-start justify-between gap-4" data-ui="InputRow">
      <div className="min-w-0 flex-1">
        <p className="text-sm text-surface-100 select-none">{title}</p>
        <p className="text-xs text-surface-400 select-none">{description}</p>
        {path && <p className="mt-1 font-mono text-xs break-all text-surface-200">{path}</p>}
        {!path && <p className="mt-1 text-xs text-surface-300 select-none">{none}</p>}
        {facts && <div className="mt-0.5 text-xs text-surface-400">{facts}</div>}
      </div>
      <div className="flex shrink-0 gap-2">{actions}</div>
    </div>
  );
}

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

  return (
    <>
      <InputRow
        title={m.inputs_game_title()}
        description={m.inputs_game_description()}
        path={view?.game.path ?? chosen["inputs.game"]}
        none={m.inputs_game_none()}
        facts={
          <>
            {view?.game.build && <p>{m.inputs_game_build_label({ build: view.game.build })}</p>}
            {view && view.game.unreadable_wads > 0 && (
              <p className="text-warning-text">
                {m.inputs_game_unreadable_wads_label({ count: view.game.unreadable_wads })}
              </p>
            )}
            {find.isSuccess && !find.data && (
              <p className="text-warning-text">{m.inputs_find_none_hint()}</p>
            )}
          </>
        }
        actions={
          <>
            <Button size="sm" variant="outline" loading={find.isPending} onClick={findGame}>
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
        description={m.inputs_hashes_description()}
        path={view?.hashes.dir}
        none={m.inputs_hashes_none()}
        facts={
          <>
            {!chosen["inputs.hashes"] && view?.hashes.dir && (
              <p>{m.inputs_hashes_manager_label()}</p>
            )}
            {view && view.hashes.tables.length > 0 && (
              <p>{m.inputs_hashes_tables_label({ tables: view.hashes.tables.join(", ") })}</p>
            )}
          </>
        }
        actions={
          <>
            {chosen["inputs.hashes"] && (
              <Button size="sm" variant="ghost" onClick={() => set("inputs.hashes", null)}>
                {m.inputs_use_manager_action()}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={() => void choose("inputs.hashes")}>
              {m.inputs_choose_folder_action()}
            </Button>
          </>
        }
      />
      <InputRow
        title={m.inputs_schema_title()}
        description={m.inputs_schema_description()}
        path={view?.schema.path}
        none={m.inputs_schema_none()}
        facts={
          view?.schema.latest_build && (
            <p>{m.inputs_schema_build_label({ build: view.schema.latest_build })}</p>
          )
        }
        actions={
          <>
            {chosen["inputs.schema"] && (
              <Button size="sm" variant="ghost" onClick={() => set("inputs.schema", null)}>
                {m.inputs_use_manager_action()}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={() => void chooseSchema()}>
              {m.inputs_choose_file_action()}
            </Button>
          </>
        }
      />
      <InputRow
        title={m.inputs_history_title()}
        description={m.inputs_history_description()}
        path={view?.history.dir}
        none={m.inputs_history_none()}
        facts={
          <>
            {view?.history.builds != null && view.history.newest_build && (
              <p>
                {m.inputs_history_builds_label({
                  builds: view.history.builds,
                  build: view.history.newest_build,
                })}
              </p>
            )}
            <IndexDownload installed={view?.history} />
          </>
        }
        actions={
          <>
            {chosen["inputs.history"] && (
              <Button size="sm" variant="ghost" onClick={() => set("inputs.history", null)}>
                {m.inputs_history_use_none_action()}
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={() => void choose("inputs.history")}>
              {m.inputs_choose_folder_action()}
            </Button>
          </>
        }
      />
    </>
  );
}

/** The inputs a run is judged against: each one, where it was read from, and what was found. */
export function InputsPage() {
  const inputs = useInputs();

  return (
    <div className="flex max-w-3xl flex-col gap-6" data-ui="InputsPage">
      <div>
        <h1 className="font-display text-xl font-semibold text-surface-50">
          {m.inputs_page_title()}
        </h1>
        <p className="text-sm text-surface-400">{m.inputs_page_description()}</p>
      </div>
      <SectionCard title={m.inputs_page_title()}>
        <Rows view={inputs.data} />
        {inputs.isFetching && (
          <p className="flex items-center gap-2 text-xs text-surface-400 select-none">
            <Spinner size="sm" />
            {m.inputs_opening_label()}
          </p>
        )}
        {inputs.isError && <FailureNote failure={inputs.error} />}
      </SectionCard>
      {inputs.data && (
        <SectionCard title={m.inputs_findings_title()}>
          <FindingList findings={inputs.data.findings} empty={m.inputs_findings_empty()} />
        </SectionCard>
      )}
    </div>
  );
}
