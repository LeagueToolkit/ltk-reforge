/**
 * A row's value, read-only, drawn as the editor draws it at rest.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/values/components/RowValue.tsx` (`Value`
 * and the widgets it draws), with `Readout` and `ChannelSash` from its `src/components/`. Cut:
 * every edit field, the enums and units a field's tables give, the link chips (a link draws as
 * its text, its hash on hover), the class cards, the ranges of a random value. A colour vector
 * draws its swatch beside its components, where Manager's colour field draws a swatch and its
 * hex.
 */
import { CaretRightIcon } from "@phosphor-icons/react";
import { type MouseEvent as ReactMouseEvent, type ReactNode, useState } from "react";

import { m } from "@/i18n";
import type { BinRow, BinValue, RowNode } from "@/lib/bindings.gen";
import { twMerge } from "@/utils";

import { useValueMark } from "../hooks/useValueMarks";
import { rowKey } from "../rows";
import { channels, colorStops, markValue, type ValueMark } from "../valueRows";
import { ColorMark, Swatch } from "./ColorMark";

const AXES = ["x", "y", "z", "w"] as const;
/** The room a number on its own takes, so a column of rows lines its digits up. */
const SCALAR_CLASS = "w-32";
/** One component of a vector or a matrix, which holds a float. */
const COMPONENT_CLASS = "w-24";

/* DS-KIND-HUE, DS-TEXT: a class reads in the class hue, as the editor's class card does. */
const CLASS_CLASS = "min-w-0 truncate text-bin-class-text select-text";

/* DS-KIND-HUE: X red, Y green, Z blue and W as Riot draws them, and R, G, B, A the same. */
const SASH_CLASSES = ["bg-channel-1", "bg-channel-2", "bg-channel-3", "bg-channel-4"] as const;

/** The cell a row's value draws. */
export function RowValue({ row, className }: { row: BinRow; className?: string }) {
  const { value } = row;
  const rgba = value.type === "vector" && isColorVector(row.name, value.values);
  return (
    <span className={twMerge("flex min-w-0 flex-1 items-center gap-2", className)}>
      {rgba && <ColorSwatchOf value={value} />}
      <Value value={value} node={row.node} rowKey={rowKey(row)} />
    </span>
  );
}

/** Whether a float vector named `name` holds a colour: three or four channels, named for one. */
export function isColorVector(name: string, values: readonly unknown[]): boolean {
  return (values.length === 3 || values.length === 4) && /colou?r$/i.test(name);
}

/** The swatch of a float vector a colour's name holds, where every channel is a number. */
function ColorSwatchOf({ value }: { value: BinValue }) {
  const rgba = channels(value);
  return rgba === null ? null : <Swatch rgba={rgba} />;
}

interface ValueProps {
  value: BinValue;
  /** Where the row sits, which decides what the name cell already drew. */
  node: RowNode;
  /** The row's own key, which a value the projected read answers for reads its mark under. */
  rowKey: string;
}

function Value({ value, node, rowKey: key }: ValueProps) {
  switch (value.type) {
    case "none":
      return <Dim>{m.bin_none_label()}</Dim>;
    case "bool":
      return <BoolValue value={value.value} />;
    case "integer":
      return <Readout value={value.text} className={SCALAR_CLASS} />;
    case "float":
      return <Readout value={String(value.value)} className={SCALAR_CLASS} />;
    case "vector":
      return <Components values={value.values} />;
    case "matrix":
      return <MatrixValue values={value.values} />;
    case "color":
      return <ByteColor value={value} />;
    case "string":
      return <Text title={value.value}>{value.value}</Text>;
    case "hash":
    case "objectLink":
      return (
        <Text title={value.hash} mono={value.name === null}>
          {value.name ?? value.hash}
        </Text>
      );
    case "wadChunkLink":
      return (
        <Text title={value.hash} mono={value.path === null}>
          {value.path ?? value.hash}
        </Text>
      );
    case "container":
      if (value.len === 0) return <Dim>{m.bin_empty_label()}</Dim>;
      return <Dim>{m.bin_items_label({ count: value.len })}</Dim>;
    case "map":
      if (value.len === 0) return <Dim>{m.bin_empty_label()}</Dim>;
      return <Dim>{m.bin_entries_label({ count: value.len })}</Dim>;
    case "struct":
      return <StructValue value={value} node={node} rowKey={key} />;
    case "null":
      return <Dim>{m.bin_null_label()}</Dim>;
    case "optional":
      if (value.present) return <Dim>{m.bin_present_label()}</Dim>;
      return <Dim>{m.bin_absent_label()}</Dim>;
    case "undrawn":
      return <Dim>{m.bin_undrawn_label()}</Dim>;
    case "records":
      return <span className="ml-auto text-meta text-surface-400">{value.len}</span>;
  }
}

interface StructValueProps {
  value: Extract<BinValue, { type: "struct" }>;
  node: RowNode;
  rowKey: string;
}

function StructValue({ value, node, rowKey: key }: StructValueProps) {
  const mark = useValueMark(key);

  /* An element names its class beside its index, so the value column would write it twice. */
  if (node === "element") return <ValueMarkCell mark={mark} />;

  return (
    <>
      <ClassName classHash={value.classHash} name={value.class} />
      <ValueMarkCell mark={mark} />
      {node === "object" && <PropertyCount count={value.len} />}
    </>
  );
}

/** A class by its name, or its hash where nothing names it, the hash on hover. */
export function ClassName({ classHash, name }: { classHash: string; name: string | null }) {
  return (
    <span title={classHash} className={twMerge(CLASS_CLASS, name === null && "font-mono")}>
      {name ?? classHash}
    </span>
  );
}

/** How many properties an object holds, beside its class. */
function PropertyCount({ count }: { count: number }) {
  return (
    <span
      title={m.bin_object_property_count_label({ count })}
      className="flex shrink-0 items-center gap-1.5 text-meta text-surface-400"
    >
      <span aria-hidden>·</span>
      {count}
    </span>
  );
}

/**
 * The constant a value-family row draws beside its class, once the read lands.
 *
 * Nothing until it lands, which keeps the row one line rather than a placeholder that shifts.
 */
function ValueMarkCell({ mark }: { mark: ValueMark | undefined }) {
  if (mark === undefined) return null;
  /* A colour that animates is drawn by its stops, which a file writing no constant still has. */
  if (mark.family === "color") {
    const rgba = channels(mark.constant);
    const stops = colorStops(mark.keys);
    if (rgba === null && stops.length === 0) return null;
    return <ColorMark constant={rgba} stops={stops} />;
  }
  const value = markValue(mark);
  if (value?.type === "float")
    return <Readout value={String(value.value)} className={SCALAR_CLASS} />;
  if (value?.type === "vector") return <Components values={value.values} />;
  return null;
}

/**
 * A value in the box it would be edited in, read-only.
 *
 * The box is drawn at rest rather than on hover, so a value reads as something the bin holds
 * rather than as text laid over the row. It takes no focus, since a bin of them would be a tab
 * order thousands of stops long.
 */
function Readout({
  value,
  channel,
  className,
}: {
  value: string;
  /** The component's channel, whose sash heads the box. */
  channel?: number;
  className?: string;
}) {
  const field = (
    <span
      data-ui="Readout"
      className={twMerge(
        "min-w-0 truncate bg-surface-veil-soft px-1.5 py-0.5 font-mono text-surface-200 tabular-nums select-text",
        channel === undefined && "rounded-sm border border-surface-veil",
        className,
      )}
    >
      {value}
    </span>
  );
  if (channel === undefined) return field;
  return (
    <span className="inline-flex min-w-0 items-stretch overflow-hidden rounded-sm border border-surface-veil">
      <span
        aria-hidden
        className={twMerge(
          "w-0.75 shrink-0 self-stretch",
          SASH_CLASSES[channel] ?? "bg-surface-500",
        )}
      />
      {field}
    </span>
  );
}

/** A vector's components, each headed by its channel's sash. */
function Components({ values }: { values: readonly (number | null)[] }) {
  return (
    <span className="flex min-w-0 gap-1.5">
      {values.map((component, at) => (
        <Readout
          key={AXES[at] ?? at}
          value={String(component)}
          channel={at}
          className={COMPONENT_CLASS}
        />
      ))}
    </span>
  );
}

/** Sixteen cells, shut until asked for. A shut matrix is one line like every other row. */
function MatrixValue({ values }: { values: readonly (number | null)[] }) {
  const [open, setOpen] = useState(false);
  const label = m.bin_matrix_label();

  function toggle(event: ReactMouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    setOpen((shown) => !shown);
  }

  if (!open) {
    return (
      <button
        type="button"
        tabIndex={-1}
        className="flex cursor-pointer items-center gap-1 text-surface-400 hover:text-surface-200"
        onClick={toggle}
      >
        <CaretRightIcon weight="bold" className="size-3" />
        <span>{label}</span>
      </button>
    );
  }

  return (
    <span className="my-1 flex items-start gap-1">
      <button
        type="button"
        tabIndex={-1}
        aria-label={label}
        className="mt-1 flex h-4 w-3 shrink-0 cursor-pointer items-center justify-center text-surface-400 hover:text-surface-200"
        onClick={toggle}
      >
        <CaretRightIcon weight="bold" className="size-3 rotate-90" />
      </button>
      <span className="grid grid-cols-4 gap-x-1 gap-y-0.5">
        {values.map((cell, at) => (
          <Readout key={at} value={String(cell)} className={COMPONENT_CLASS} />
        ))}
      </span>
    </span>
  );
}

/** A colour of four bytes: its swatch, and its channels. */
function ByteColor({ value }: { value: Extract<BinValue, { type: "color" }> }) {
  const { r, g, b, a } = value;
  return (
    <span className="flex min-w-0 items-center gap-2">
      <Swatch rgba={[r / 255, g / 255, b / 255, a / 255]} />
      <Components values={[r, g, b, a]} />
    </span>
  );
}

/** A bool as a box ticked or not. */
function BoolValue({ value }: { value: boolean }) {
  return (
    <span
      role="img"
      aria-label={String(value)}
      className={twMerge(
        "flex size-3.5 shrink-0 items-center justify-center rounded-sm border",
        value ? "border-accent-500 bg-accent-500" : "border-surface-600 bg-surface-950/40",
      )}
    >
      {value && <span className="size-1.5 rounded-[1px] bg-surface-900" />}
    </span>
  );
}

function Text({
  children,
  title,
  mono = false,
}: {
  children: ReactNode;
  title: string;
  mono?: boolean;
}) {
  return (
    <span
      title={title}
      className={twMerge(
        "min-w-0 truncate text-surface-200 select-text",
        mono && "text-surface-300",
      )}
    >
      {children}
    </span>
  );
}

function Dim({ children }: { children: ReactNode }) {
  return <span className="text-surface-400">{children}</span>;
}
