import { FloppyDiskIcon, FolderOpenIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react";
import { useMutation } from "@tanstack/react-query";
import { useState } from "react";

import { Button, FailureNote, SectionCard, SegmentedControl } from "@/components";
import { m } from "@/i18n";
import { commands, type FieldRow, type RuleOp, type RuleSpec } from "@/lib/bindings.gen";
import { pickFile, pickSavePath } from "@/modules/inputs";
import { unwrap } from "@/utils";

import { useRepairStore } from "../state/repairStore";

type OpName = RuleOp["op"];

const inputClass =
  "rounded-md border border-surface-600 bg-surface-950/40 px-2 py-1 font-mono text-xs text-surface-100 focus-visible:outline-2 focus-visible:outline-accent-500";

/** Rows written one per line, `from -> to`, or one name for a field that keeps it. */
export function parseRows(text: string): FieldRow[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const [from, to] = line.split("->").map((part) => part.trim());
      return { from, to: to || from };
    });
}

/** A rule from the editor's fields. */
export function ruleOf(draft: Draft): RuleSpec {
  const op: RuleOp =
    draft.op === "remove"
      ? { op: "remove", fields: parseRows(draft.fields).map((row) => row.from) }
      : draft.op === "rename"
        ? { op: "rename", fields: parseRows(draft.fields) }
        : {
            op: "move",
            into: { field: draft.childField.trim(), class: draft.childClass.trim() },
            fields: parseRows(draft.fields),
          };
  return {
    id: draft.id.trim(),
    title: draft.title.trim() || null,
    records: [],
    phase: { name: "undo" },
    default: true,
    class: draft.class.trim(),
    op,
  };
}

/** What the rule editor holds. */
export interface Draft {
  id: string;
  title: string;
  class: string;
  op: OpName;
  fields: string;
  childField: string;
  childClass: string;
}

const EMPTY: Draft = {
  id: "",
  title: "",
  class: "",
  op: "remove",
  fields: "",
  childField: "",
  childClass: "",
};

function RuleEditor({ onAdd }: { onAdd: (rule: RuleSpec) => void }) {
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const set = (change: Partial<Draft>) => setDraft({ ...draft, ...change });
  const ops: { value: OpName; label: string }[] = [
    { value: "remove", label: m.rules_op_remove_label() },
    { value: "rename", label: m.rules_op_rename_label() },
    { value: "move", label: m.rules_op_move_label() },
  ];
  const ready = draft.id.trim() && draft.class.trim() && draft.fields.trim();

  return (
    <div
      className="flex flex-col gap-2 rounded-lg border border-surface-700 p-3"
      data-ui="RuleEditor"
    >
      <div className="grid grid-cols-2 gap-2">
        <label className="flex flex-col gap-1 text-xs text-surface-300">
          {m.rules_id_label()}
          <input
            className={inputClass}
            value={draft.id}
            onChange={(e) => set({ id: e.target.value })}
          />
        </label>
        <label className="flex flex-col gap-1 text-xs text-surface-300">
          {m.rules_class_label()}
          <input
            className={inputClass}
            value={draft.class}
            onChange={(e) => set({ class: e.target.value })}
          />
        </label>
      </div>
      <label className="flex flex-col gap-1 text-xs text-surface-300">
        {m.rules_title_label()}
        <input
          className={inputClass}
          value={draft.title}
          onChange={(e) => set({ title: e.target.value })}
        />
      </label>
      <SegmentedControl
        options={ops}
        value={draft.op}
        onChange={(op) => set({ op })}
        size="xs"
        aria-label={m.rules_op_label()}
      />
      {draft.op === "move" && (
        <div className="grid grid-cols-2 gap-2">
          <label className="flex flex-col gap-1 text-xs text-surface-300">
            {m.rules_child_field_label()}
            <input
              className={inputClass}
              value={draft.childField}
              onChange={(e) => set({ childField: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs text-surface-300">
            {m.rules_child_class_label()}
            <input
              className={inputClass}
              value={draft.childClass}
              onChange={(e) => set({ childClass: e.target.value })}
            />
          </label>
        </div>
      )}
      <label className="flex flex-col gap-1 text-xs text-surface-300">
        {draft.op === "remove" ? m.rules_fields_remove_label() : m.rules_fields_rows_label()}
        <textarea
          rows={3}
          className={inputClass}
          value={draft.fields}
          onChange={(e) => set({ fields: e.target.value })}
        />
      </label>
      <div className="flex justify-end">
        <Button
          size="sm"
          variant="filled"
          left={<PlusIcon weight="bold" />}
          disabled={!ready}
          onClick={() => {
            onAdd(ruleOf(draft));
            setDraft(EMPTY);
          }}
        >
          {m.rules_add_action()}
        </Button>
      </div>
    </div>
  );
}

function opSummary(rule: RuleSpec): string {
  const fields = rule.op.op === "remove" ? rule.op.fields : rule.op.fields.map((row) => row.from);
  return `${rule.op.op} ${fields.join(", ")}`;
}

/**
 * The rules added to a run, and the choice of fixups saved to or loaded from a selection file:
 * the file `reforge` reads with `--fixups`.
 */
export function RulesCard() {
  const advanced = useRepairStore((state) => state.advanced);
  const setAdvanced = useRepairStore((state) => state.setAdvanced);
  const choice = { off: advanced.fixupsOff, on: advanced.fixupsOn, rules: advanced.rules };

  // Each asks the user for a file first, and does nothing when none is picked.
  const save = useMutation({
    mutationFn: async () => {
      const path = await pickSavePath(["json"], m.rules_selection_filter_label());
      if (path) {
        unwrap(await commands.saveSelection(path, choice));
      }
    },
  });
  const load = useMutation({
    mutationFn: async () => {
      const path = await pickFile(["json"], m.rules_selection_filter_label());
      return path ? unwrap(await commands.loadSelection(path)) : null;
    },
    onSuccess: (loaded) => {
      if (loaded) {
        setAdvanced({
          fixupsOff: loaded.off ?? [],
          fixupsOn: loaded.on ?? [],
          rules: loaded.rules ?? [],
        });
      }
    },
  });

  return (
    <SectionCard
      title={m.rules_title()}
      description={m.rules_description()}
      action={
        <div className="flex gap-2">
          <Button
            size="xs"
            variant="outline"
            left={<FolderOpenIcon weight="bold" />}
            loading={load.isPending}
            onClick={() => load.mutate()}
          >
            {m.rules_load_action()}
          </Button>
          <Button
            size="xs"
            variant="outline"
            left={<FloppyDiskIcon weight="bold" />}
            loading={save.isPending}
            onClick={() => save.mutate()}
          >
            {m.rules_save_action()}
          </Button>
        </div>
      }
    >
      {load.isError && <FailureNote failure={load.error} />}
      {save.isError && <FailureNote failure={save.error} />}
      {advanced.rules.length > 0 && (
        <ul className="divide-y divide-surface-700/50">
          {advanced.rules.map((rule) => (
            <li key={rule.id} className="flex items-start justify-between gap-3 py-2">
              <div className="min-w-0">
                <p className="text-sm text-surface-100">{rule.title ?? rule.id}</p>
                <p className="font-mono text-xs break-all text-surface-400">
                  {rule.id} · {rule.class} · {opSummary(rule)}
                </p>
              </div>
              <Button
                size="xs"
                variant="ghost"
                left={<TrashIcon weight="bold" />}
                aria-label={m.rules_remove_action()}
                onClick={() =>
                  setAdvanced({ rules: advanced.rules.filter((other) => other.id !== rule.id) })
                }
              />
            </li>
          ))}
        </ul>
      )}
      <RuleEditor
        onAdd={(rule) =>
          setAdvanced({
            rules: [...advanced.rules.filter((other) => other.id !== rule.id), rule],
          })
        }
      />
    </SectionCard>
  );
}
