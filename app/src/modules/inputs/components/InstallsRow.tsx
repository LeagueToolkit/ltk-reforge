import { CircleIcon, RadioButtonIcon, XIcon } from "@phosphor-icons/react";
import type { KeyboardEvent } from "react";

import { Button, FailureNote, IconButton, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { InputsView, InstallSource, InstallView } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { useAddInstall, useChooseInstall, useInstalls, useRemoveInstall } from "../api/installs";
import { pickFolder } from "../api/pick";
import { InputRow, PathLine, StatusBadge } from "./InputRow";

const sourceLabels: Record<InstallSource, () => string> = {
  live: m.inputs_installs_source_live_label,
  pbe: m.inputs_installs_source_pbe_label,
  running: m.inputs_installs_source_running_label,
  added: m.inputs_installs_source_added_label,
};

/** Where an install was found, in a word. */
function SourceBadge({ source }: { source: InstallSource }) {
  return (
    <span
      className="inline-flex items-center rounded-full bg-surface-veil px-2 py-0.5 text-xs font-medium whitespace-nowrap text-surface-300 select-none"
      data-ui="SourceBadge"
      data-source={source}
    >
      {sourceLabels[source]()}
    </span>
  );
}

interface InstallLineProps {
  install: InstallView;
  /** How many WADs of the chosen install did not read, once the inputs opened. */
  unreadableWads: number;
  onChoose: (path: string) => void;
  onRemove: (path: string) => void;
}

/** One install of the list: chosen by a click on it. */
function InstallLine({ install, unreadableWads, onChoose, onRemove }: InstallLineProps) {
  const choose = () => {
    if (!install.chosen) {
      onChoose(install.path);
    }
  };
  const onKeyDown = (event: KeyboardEvent) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      choose();
    }
  };
  const Mark = install.chosen ? RadioButtonIcon : CircleIcon;
  return (
    <div
      role="radio"
      aria-checked={install.chosen}
      tabIndex={0}
      onClick={choose}
      onKeyDown={onKeyDown}
      className={twMerge(
        "flex cursor-pointer flex-col gap-1.5 rounded-lg border px-3 py-2 transition-colors duration-150 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-500",
        install.chosen
          ? "border-accent-500/60 bg-surface-veil-soft"
          : "border-surface-veil-strong hover:bg-surface-veil-soft",
      )}
      data-ui="InstallLine"
    >
      <div className="flex items-center gap-2 text-xs select-none">
        <Mark
          weight={install.chosen ? "fill" : "bold"}
          className={twMerge(
            "h-4 w-4 shrink-0",
            install.chosen ? "text-accent-400" : "text-surface-400",
          )}
        />
        <SourceBadge source={install.source} />
        {install.build && (
          <span className="text-surface-400">
            {m.inputs_game_build_label({ build: install.build })}
          </span>
        )}
        {!install.readable && (
          <span className="text-danger-text">{m.inputs_installs_unreadable_label()}</span>
        )}
        {install.chosen && unreadableWads > 0 && (
          <span className="text-warning-text">
            {m.inputs_game_unreadable_wads_label({ count: unreadableWads })}
          </span>
        )}
        <span className="flex-1" />
        {install.source === "added" && (
          <Tooltip content={m.inputs_installs_remove_action()}>
            <IconButton
              size="xs"
              compact
              variant="ghost"
              aria-label={m.inputs_installs_remove_action()}
              icon={<XIcon weight="bold" />}
              onClick={(event) => {
                event.stopPropagation();
                onRemove(install.path);
              }}
            />
          </Tooltip>
        )}
      </div>
      {/* The folder's button shows the folder and chooses nothing. */}
      <div
        onClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => event.stopPropagation()}
      >
        <PathLine path={install.path} place={{ install: { path: install.path } }} />
      </div>
    </div>
  );
}

/**
 * The game installs (#ADR-0101): the ones detected on this machine, live and PBE, and the ones
 * added, one of them chosen for every run.
 */
export function InstallsRow({ view }: { view: InputsView | undefined }) {
  const installs = useInstalls();
  const add = useAddInstall();
  const remove = useRemoveInstall();
  const choose = useChooseInstall();

  const list = installs.data ?? [];
  const chosen = list.find((install) => install.chosen);
  const addFolder = async () => {
    const picked = await pickFolder();
    if (picked) {
      add.mutate(picked);
    }
  };
  const badge = !chosen ? (
    <StatusBadge tone="bad">{m.inputs_status_not_chosen_label()}</StatusBadge>
  ) : (
    !chosen.readable && <StatusBadge tone="bad">{m.inputs_status_unreadable_label()}</StatusBadge>
  );
  const failure = installs.error ?? add.error ?? remove.error ?? choose.error;

  return (
    <InputRow
      title={m.inputs_game_title()}
      badge={installs.isSuccess && badge}
      description={m.inputs_game_description()}
      actions={
        <>
          <Button
            size="sm"
            variant="ghost"
            loading={installs.isFetching}
            onClick={() => void installs.refetch()}
          >
            {m.inputs_installs_find_action()}
          </Button>
          <Button
            size="sm"
            variant="outline"
            loading={add.isPending}
            onClick={() => void addFolder()}
          >
            {m.inputs_installs_add_action()}
          </Button>
        </>
      }
    >
      {installs.isSuccess && list.length === 0 && (
        <p className="text-xs text-surface-300 select-none">{m.inputs_installs_none()}</p>
      )}
      {list.length > 0 && (
        <div
          role="radiogroup"
          aria-label={m.inputs_installs_list_label()}
          className="flex flex-col gap-2"
        >
          {list.map((install) => (
            <InstallLine
              key={install.path}
              install={install}
              unreadableWads={view?.game.path === install.path ? view.game.unreadable_wads : 0}
              onChoose={(path) => choose.mutate(path)}
              onRemove={(path) => remove.mutate(path)}
            />
          ))}
        </div>
      )}
      {failure && <FailureNote failure={failure} className="text-xs" />}
    </InputRow>
  );
}
