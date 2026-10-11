import { FileMagnifyingGlassIcon } from "@phosphor-icons/react";
import { useQuery } from "@tanstack/react-query";
import { useContext, useMemo, useState } from "react";

import { Button, Disclosure, FailureNote, Spinner } from "@/components";
import { m } from "@/i18n";
import type { RunRef, ValuePlaceView } from "@/lib/bindings.gen";
import { FileJumpContext, type FilesQuery, placeRow } from "@/modules/files";

import { useRunRef, useSpelled, useTypePlaces } from "../api/run";
import {
  fieldsOf,
  type FirstPlace,
  parseFieldPath,
  pathText,
  rowHashes,
  shapeText,
  type TypeFinding,
  type TypeList,
  type TypeRow,
} from "../types";

/** How many places a row shows at first, and adds with each "show more". */
const PAGE = 20;

/** A hash's name where one is spelled, else the hash. */
type Namer = (hash: string) => string;

function namer(spelled: Record<string, string>): Namer {
  return (hash) => spelled[hash] ?? hash;
}

/** A row's words, the class and field as `Class.field`, each hash by its name. */
function rowText(row: TypeRow, list: TypeList, name: Namer): string {
  const field = `${name(row.class ?? "")}.${name(row.field ?? "")}`;
  if (list === "mismatched" && row.found && row.wants) {
    return m.findings_type_mismatched_row({
      field,
      found: shapeText(row.found, name),
      wants: shapeText(row.wants, name),
    });
  }
  if (list === "classes") {
    const held = name(row.held ?? "");
    return row.field === undefined
      ? m.findings_type_class_object_row({ held })
      : m.findings_type_class_held_row({ field, held });
  }
  return field;
}

/** A place as `WAD · bin path · entry`, the entry by its name. */
function placeText(place: FirstPlace, name: Namer): string {
  return `${place.wad} · ${place.path} · ${name(place.entry)}`;
}

/** What a row counts: values, or structs for an undeclared class. */
function countText(row: TypeRow, list: TypeList): string {
  return list === "classes"
    ? m.findings_type_structs({ count: row.count })
    : m.findings_type_values({ count: row.count });
}

interface RowProps {
  row: TypeRow;
  list: TypeList;
  name: Namer;
}

/** A row in a line, its hashes as hex in the title, and its first place under it. */
function RowSummary({ row, list, name }: RowProps) {
  const hex: Namer = (hash) => hash;
  return (
    <span className="flex flex-col">
      <span className="break-all text-surface-100" title={rowText(row, list, hex)}>
        {rowText(row, list, name)}{" "}
        <span className="text-surface-400 tabular-nums">({countText(row, list)})</span>
      </span>
      <span className="break-all text-surface-400" title={placeText(row.first, hex)}>
        {m.findings_type_first({ place: placeText(row.first, name) })}
      </span>
    </span>
  );
}

/** A jump to a place's object in its bin, or none. */
type ShowInBin = (place: FirstPlace) => (() => void) | null;

/** The read of no files, for a page with no jump. */
const NO_FILES: FilesQuery = { queryKey: ["no-files"], queryFn: () => Promise.resolve(null) };

/**
 * The jump to a place's object in its bin, through the page's files at `build`. None on a page
 * with no jump, and for a place in no file of the package, such as a game bin a patch changes.
 */
function useShowInBin(): ShowInBin {
  const jump = useContext(FileJumpContext);
  const build = useQuery({
    ...(jump?.build ?? NO_FILES),
    enabled: jump != null,
    staleTime: Infinity,
  });
  return (place) => {
    const row = jump && build.data ? placeRow(build.data, place.wad, place.path) : null;
    return jump && row ? () => jump.open(row, place.entry) : null;
  };
}

/** The button that shows a place's object in its bin. */
function ShowButton({ show }: { show: (() => void) | null }) {
  if (!show) {
    return null;
  }
  return (
    <Button
      size="xs"
      compact
      variant="ghost"
      left={<FileMagnifyingGlassIcon weight="bold" />}
      className="ml-2 font-sans"
      onClick={show}
    >
      {m.findings_show_in_bin_action()}
    </Button>
  );
}

/** One value's place: `WAD · bin path · entry · path`, each hash by its name. */
function PlaceLine({ place, name, show }: { place: ValuePlaceView; name: Namer; show: ShowInBin }) {
  const steps = parseFieldPath(place.at);
  const at = steps ? pathText(steps, name) : place.at;
  return (
    <li className="break-all text-surface-200">
      <span title={`${place.entry} · ${place.at}`}>
        {placeText(place, name)} · <span className="text-surface-50">{at}</span>
      </span>
      <ShowButton show={show(place)} />
    </li>
  );
}

/** A row's places, a page at a time, their entries and fields spelled. */
function PlaceList({
  run,
  places,
  count,
}: {
  run: RunRef;
  places: ValuePlaceView[];
  count: number;
}) {
  const [shown, setShown] = useState(PAGE);
  const page = useMemo(() => places.slice(0, shown), [places, shown]);
  const hashes = useMemo(
    () => page.flatMap((place) => [place.entry, ...fieldsOf(parseFieldPath(place.at) ?? [])]),
    [page],
  );
  const name = namer(useSpelled(run, hashes));
  const show = useShowInBin();
  const left = places.length - shown;
  return (
    <div className="flex flex-col gap-1">
      {places.length < count && (
        <p className="font-sans text-surface-400 select-none">
          {m.findings_places_capped({ shown: places.length, count })}
        </p>
      )}
      <ul className="flex flex-col gap-0.5">
        {page.map((place, at) => (
          <PlaceLine key={at} place={place} name={name} show={show} />
        ))}
      </ul>
      {left > 0 && (
        <button
          type="button"
          onClick={() => setShown(shown + PAGE)}
          className="cursor-pointer self-start font-sans text-surface-400 hover:text-surface-100"
        >
          {m.findings_more_action({ count: Math.min(PAGE, left) })}
        </button>
      )}
    </div>
  );
}

interface RowPlacesProps {
  run: RunRef;
  list: TypeList;
  /** The row's index in its finding. */
  index: number;
  /** How many rows the finding holds. */
  rows: number;
  row: TypeRow;
  name: Namer;
}

/**
 * A row's places, read from the run's package again when the row opens. The package gives the
 * finding's rows in the finding's order; when it gives another count, the row's first place
 * is all it shows.
 */
function RowPlaces({ run, list, index, rows, row, name }: RowPlacesProps) {
  const places = useTypePlaces(run);
  const show = useShowInBin();
  if (places.isPending) {
    return (
      <p className="flex items-center gap-2 font-sans text-surface-400 select-none">
        <Spinner size="sm" />
        {m.findings_places_reading()}
      </p>
    );
  }
  if (places.isError) {
    return <FailureNote failure={places.error} />;
  }
  const listed = places.data[list];
  if (listed.length !== rows) {
    return (
      <div className="flex flex-col gap-1">
        <p className="font-sans text-surface-400 select-none">{m.findings_places_differ()}</p>
        <p className="break-all text-surface-200">
          {placeText(row.first, name)}
          <ShowButton show={show(row.first)} />
        </p>
      </div>
    );
  }
  return <PlaceList run={run} places={listed[index].places} count={row.count} />;
}

/**
 * A type check's finding (`check.type-mismatch`, `check.field-undeclared`,
 * `check.class-undeclared`): a row per class and field with its count and first place, each
 * hash by the name the run's names or the tables give it. With a run to ask, a row opens to
 * the place of each of its values (#ADR-0103).
 */
export function TypeDetail({ found }: { found: TypeFinding }) {
  const run = useRunRef();
  const hashes = useMemo(() => rowHashes(found.rows), [found]);
  const name = namer(useSpelled(run, hashes));
  return (
    <ul className="flex flex-col gap-1 font-mono text-xs" data-ui="TypeDetail">
      {found.rows.map((row, index) => (
        <li key={index}>
          {run ? (
            <Disclosure
              summary={<RowSummary row={row} list={found.list} name={name} />}
              panelClassName="pt-1 pb-2 pl-5"
            >
              <RowPlaces
                run={run}
                list={found.list}
                index={index}
                rows={found.rows.length}
                row={row}
                name={name}
              />
            </Disclosure>
          ) : (
            <span className="block pl-5">
              <RowSummary row={row} list={found.list} name={name} />
            </span>
          )}
        </li>
      ))}
    </ul>
  );
}
