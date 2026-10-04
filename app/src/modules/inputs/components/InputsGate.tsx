import { WarningIcon, XCircleIcon } from "@phosphor-icons/react";
import { Link } from "@tanstack/react-router";
import { type ReactNode, useState } from "react";

import { Button, Dialog, Switch } from "@/components";
import { m } from "@/i18n";
import type { InputsView, Settings } from "@/lib/bindings.gen";
import { useSaveSettings, useSettings } from "@/modules/settings";
import { isAppError } from "@/utils";

import { useInputs } from "../api/inputs";
import { IndexDownload } from "./IndexDownload";

/** The inputs a run lacks. */
export interface Missing {
  /** No game: no run. */
  game: boolean;
  index: boolean;
  hashes: boolean;
  schema: boolean;
}

/** What the inputs, as opened or as they failed to open, lack. */
export function missingOf(view: InputsView | undefined, failure: unknown): Missing {
  const noGame =
    isAppError(failure) && (failure.code === "NO_GAME" || failure.code === "GAME_UNREADABLE");
  return {
    game: noGame,
    index: view != null && view.history.dir == null,
    hashes: view != null && view.hashes.tables.length === 0,
    schema: view != null && view.schema.path == null,
  };
}

/** Whether a run should ask before it starts. */
export function asksFirst(missing: Missing, settings: Settings | undefined): boolean {
  if (missing.game) {
    return true;
  }
  const less = missing.index || missing.hashes || missing.schema;
  return less && !settings?.["prompts.runWithLess"];
}

function Row({ icon, text, children }: { icon: ReactNode; text: string; children?: ReactNode }) {
  return (
    <li className="flex items-start gap-3">
      <span className="mt-0.5 shrink-0">{icon}</span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <p className="text-sm text-surface-100">{text}</p>
        {children}
      </div>
    </li>
  );
}

/**
 * A run's start, with the inputs checked first: a run with no game asks for one, and a run
 * with less than every input says what it lacks and offers the download it can make.
 */
export function useInputsGate() {
  const inputs = useInputs();
  const settings = useSettings();
  const save = useSaveSettings();
  const [pending, setPending] = useState<(() => void) | null>(null);
  const [skip, setSkip] = useState(false);
  const missing = missingOf(inputs.data, inputs.error);

  const gate = (start: () => void) => {
    if (asksFirst(missing, settings.data)) {
      setPending(() => start);
    } else {
      start();
    }
  };
  const close = () => setPending(null);
  const runAnyway = () => {
    if (skip && settings.data) {
      save.mutate({ ...settings.data, "prompts.runWithLess": true });
    }
    pending?.();
    close();
  };

  const warn = <WarningIcon weight="bold" className="h-4 w-4 text-warning-text" />;
  const dialog = (
    <Dialog
      open={pending != null}
      onClose={close}
      title={missing.game ? m.gate_no_game_title() : m.gate_less_title()}
      description={missing.game ? undefined : m.gate_less_description()}
      footer={
        <>
          <Button variant="ghost" onClick={close}>
            {m.gate_cancel_action()}
          </Button>
          {!missing.game && (
            <Button variant="filled" onClick={runAnyway}>
              {m.gate_run_anyway_action()}
            </Button>
          )}
        </>
      }
    >
      <ul className="flex flex-col gap-3">
        {missing.game && (
          <Row
            icon={<XCircleIcon weight="bold" className="h-4 w-4 text-danger-text" />}
            text={m.gate_game_text()}
          >
            <Link to="/inputs" onClick={close} className="text-sm text-accent-300 underline">
              {m.gate_inputs_link()}
            </Link>
          </Row>
        )}
        {missing.index && (
          <Row icon={warn} text={m.gate_index_text()}>
            <IndexDownload installed={inputs.data?.history} />
          </Row>
        )}
        {missing.hashes && <Row icon={warn} text={m.gate_hashes_text()} />}
        {missing.schema && <Row icon={warn} text={m.gate_schema_text()} />}
      </ul>
      {!missing.game && (
        <label className="flex items-center gap-2 text-sm text-surface-300 select-none">
          <Switch checked={skip} onCheckedChange={setSkip} />
          {m.gate_skip_label()}
        </label>
      )}
    </Dialog>
  );

  return { gate, dialog };
}
