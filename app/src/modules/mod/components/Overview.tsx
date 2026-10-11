import { CheckIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import { SectionCard, Tooltip } from "@/components";
import { m } from "@/i18n";
import type { DatingView, ExploreView } from "@/lib/bindings.gen";
import { kindGroup, sizeLabel, sourceLabel } from "@/modules/files";
import { twMerge } from "@/utils";

import {
  type CountRow,
  endedEmpty,
  entrySlotDiffers,
  isOneOf,
  kindRows,
  sourceRows,
} from "../model";

const tableClass = "w-full text-left text-xs";
const headClass = "pb-2 pr-3 font-medium text-surface-400 select-none";
const cellClass = "py-1.5 pr-3 align-top text-surface-200";
const monoClass = twMerge(cellClass, "font-mono break-all");
const numberClass = twMerge(cellClass, "text-right tabular-nums");

const ROLES = ["champion", "locale", "other"] as const;
const LAYOUTS = ["loose", "packed"] as const;
const CHARACTER_KINDS = ["champion", "sub"] as const;

function roleLabel(role: string): string {
  return isOneOf(ROLES, role) ? m[`mod_wad_role.${role}.label`]() : role;
}

function layoutLabel(layout: string): string {
  return isOneOf(LAYOUTS, layout) ? m[`mod_wad_layout.${layout}.label`]() : layout;
}

function characterKindLabel(kind: string): string {
  return isOneOf(CHARACTER_KINDS, kind) ? m[`mod_character_kind.${kind}.label`]() : kind;
}

/** The kinds of file a mod commonly holds, each with a word of its own. */
const FILE_KINDS = [
  "property-bin",
  "property-bin-override",
  "ritobin-text",
  "ritobin-json",
  "texture",
  "texture-dds",
  "png",
  "jpeg",
  "animation",
  "skeleton",
  "simple-skin",
  "static-mesh-binary",
  "static-mesh-ascii",
  "wwise-bank",
  "wwise-package",
  "ttf",
  "otf",
] as const;

/** A file kind in a person's words: its own word, else the file viewer's word for its group. */
function fileKindLabel(kind: string): string {
  return isOneOf(FILE_KINDS, kind)
    ? m[`mod_file_kind.${kind}.label`]()
    : m[`runs_file_kind.${kindGroup(kind)}.label`]();
}

/** A file kind's word, the library's name of the kind on hover. */
function FileKind({ kind }: { kind: string }) {
  return (
    <Tooltip content={<span className="font-mono text-xs">{kind}</span>}>
      <span>{fileKindLabel(kind)}</span>
    </Tooltip>
  );
}

/** A yes as a check, its word on hover. Nothing for a no. */
function Mark({ on, label }: { on: boolean; label: string }) {
  if (!on) {
    return null;
  }
  return (
    <Tooltip content={label}>
      <CheckIcon
        weight="bold"
        role="img"
        aria-label={label}
        className="h-3.5 w-3.5 text-success-text"
      />
    </Tooltip>
  );
}

/** A section's line when it holds nothing. */
function Empty({ children }: { children: ReactNode }) {
  return <p className="text-sm text-surface-400 select-none">{children}</p>;
}

/** What dated the mod, as a sentence. */
function evidenceSentence(dating: DatingView): string | null {
  const evidence = dating.evidence;
  if (!evidence || !dating.not_before) {
    return null;
  }
  const values = { name: evidence.name, build: dating.not_before };
  return evidence.kind === "object"
    ? m.mod_dating_evidence_object(values)
    : m.mod_dating_evidence_file(values);
}

function DatingSection({ dating }: { dating: DatingView | null }) {
  const sentence = dating && evidenceSentence(dating);
  return (
    <SectionCard title={m.mod_dating_title()} description={m.mod_dating_description()}>
      <div className="flex flex-col gap-1" data-ui="ModPage:dating">
        <p className="font-display text-lg font-semibold text-surface-50">
          {dating?.not_before ?? m.mod_dating_none()}
        </p>
        {sentence && <p className="text-sm break-words text-surface-200">{sentence}</p>}
        {dating && (
          <p className="text-xs text-surface-400">
            {m.mod_dating_counts({ dated: dating.dated, atBound: dating.at_bound })}
          </p>
        )}
      </div>
    </SectionCard>
  );
}

function CharactersSection({ view }: { view: ExploreView }) {
  return (
    <SectionCard title={m.mod_characters_title()}>
      {view.characters.length === 0 ? (
        <Empty>{m.mod_characters_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="ModPage:characters">
          <thead>
            <tr>
              <th className={headClass}>{m.mod_column_name()}</th>
              <th className={headClass}>{m.mod_column_kind()}</th>
              <th className={headClass}>{m.mod_column_wad()}</th>
              <th className={twMerge(headClass, "text-right")}>{m.mod_column_units()}</th>
              <th className={headClass}>{m.mod_column_equal()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {view.characters.map((character) => (
              <tr key={character.id}>
                <td className={cellClass}>{character.spelled}</td>
                <td className={cellClass}>{characterKindLabel(character.kind)}</td>
                <td className={monoClass}>{character.wad}</td>
                <td className={numberClass}>{character.units}</td>
                <td className={cellClass}>
                  <Mark on={character.equal_to_game} label={m.mod_character_equal_hint()} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </SectionCard>
  );
}

function UnitsSection({ view }: { view: ExploreView }) {
  // A unit names its character by the directory, lower case; the Characters card spells it.
  const spelled = new Map(view.characters.map((character) => [character.name, character.spelled]));
  return (
    <SectionCard title={m.mod_units_title()}>
      {view.units.length === 0 ? (
        <Empty>{m.mod_units_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="ModPage:units">
          <thead>
            <tr>
              <th className={headClass}>{m.mod_column_character()}</th>
              <th className={twMerge(headClass, "text-right")}>{m.mod_column_slot()}</th>
              <th className={headClass}>{m.mod_column_skin_bin()}</th>
              <th className={twMerge(headClass, "text-right")}>{m.mod_column_entry_slot()}</th>
              <th className={twMerge(headClass, "text-right")}>{m.mod_column_loads()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {view.units.map((unit) => (
              <tr key={unit.id}>
                <td className={cellClass}>
                  {spelled.get(unit.character_name) ?? unit.character_name}
                </td>
                <td className={numberClass}>{unit.slot}</td>
                <td className={monoClass}>{unit.file}</td>
                <td className={twMerge(numberClass, "text-warning-text")}>
                  {entrySlotDiffers(unit) ? unit.entry_name_slot : null}
                </td>
                <td className={numberClass}>{unit.loads}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </SectionCard>
  );
}

function WadsSection({ view }: { view: ExploreView }) {
  return (
    <SectionCard title={m.mod_wads_title()}>
      {view.wads.length === 0 ? (
        <Empty>{m.mod_wads_empty()}</Empty>
      ) : (
        <table className={tableClass} data-ui="ModPage:wads">
          <thead>
            <tr>
              <th className={headClass}>{m.mod_column_name()}</th>
              <th className={headClass}>{m.mod_column_role()}</th>
              <th className={headClass}>{m.mod_column_layout()}</th>
              <th className={twMerge(headClass, "text-right")}>{m.mod_column_files()}</th>
              <th className={twMerge(headClass, "text-right")}>{m.mod_column_named()}</th>
              <th className={headClass}>{m.mod_column_locale()}</th>
              <th className={headClass}>{m.mod_column_restoration()}</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-surface-700/50">
            {view.wads.map((wad) => (
              <tr key={wad.id}>
                <td className={monoClass}>{wad.name}</td>
                <td className={cellClass}>{roleLabel(wad.role)}</td>
                <td className={cellClass}>{layoutLabel(wad.layout)}</td>
                <td className={numberClass}>{wad.files}</td>
                <td className={numberClass}>{wad.named}</td>
                <td className={monoClass}>{wad.packed_locale}</td>
                <td className={cellClass}>
                  <Mark on={wad.restoration} label={m.mod_wad_restoration_hint()} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </SectionCard>
  );
}

/** A count of files and bytes per key, compact. */
function CountTable({
  rows,
  head,
  label,
  ui,
}: {
  rows: CountRow[];
  head: string;
  label: (key: string) => ReactNode;
  ui: string;
}) {
  return (
    <table className={tableClass} data-ui={ui}>
      <thead>
        <tr>
          <th className={headClass}>{head}</th>
          <th className={twMerge(headClass, "text-right")}>{m.mod_column_files()}</th>
          <th className={twMerge(headClass, "text-right")}>{m.mod_column_bytes()}</th>
        </tr>
      </thead>
      <tbody className="divide-y divide-surface-700/50">
        {rows.map((row) => (
          <tr key={row.key}>
            <td className={cellClass}>{label(row.key)}</td>
            <td className={numberClass}>{row.files}</td>
            <td className={numberClass}>{sizeLabel(row.bytes)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function CountsSection({ view }: { view: ExploreView }) {
  const kinds = kindRows(view.kinds);
  const sources = sourceRows(view.sources);
  return (
    <div className="grid gap-6 md:grid-cols-2">
      <SectionCard title={m.mod_by_kind_title()}>
        {kinds.length === 0 ? (
          <Empty>{m.mod_by_kind_empty()}</Empty>
        ) : (
          <CountTable
            rows={kinds}
            head={m.mod_column_kind()}
            label={(key) => <FileKind kind={key} />}
            ui="ModPage:by-kind"
          />
        )}
      </SectionCard>
      <SectionCard title={m.mod_by_source_title()} description={m.mod_by_source_description()}>
        {sources.length === 0 ? (
          <Empty>{m.mod_by_source_empty()}</Empty>
        ) : (
          <CountTable
            rows={sources}
            head={m.mod_column_source()}
            label={sourceLabel}
            ui="ModPage:by-source"
          />
        )}
      </SectionCard>
    </div>
  );
}

/** The load report of a mod explored: its dating, characters, units, WADs and counts. */
export function Overview({ view }: { view: ExploreView }) {
  if (endedEmpty(view)) {
    return <Empty>{m.mod_overview_ended()}</Empty>;
  }
  return (
    <div className="flex flex-col gap-6" data-ui="ModPage:overview">
      <DatingSection dating={view.dating} />
      <CharactersSection view={view} />
      <UnitsSection view={view} />
      <WadsSection view={view} />
      <CountsSection view={view} />
    </div>
  );
}
