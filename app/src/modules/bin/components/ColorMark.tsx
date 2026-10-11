/**
 * A colour as a swatch, and a curve of colours as a strip of its stops.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/values/components/ColorMark.tsx`. Cut:
 * the hover card is the app's tooltip rather than a popover, and the wide strip a field row of
 * a layout draws.
 */
import { Tooltip } from "@/components";
import { m } from "@/i18n";
import { twMerge } from "@/utils";

import { colorCss, colorHex, type ColorStop, gradientCss } from "../valueRows";

/** The checkerboard an alpha reads over, so it reads as one rather than as a tint. */
const CHECKER_CLASS =
  "bg-[repeating-conic-gradient(var(--color-surface-600)_0_25%,var(--color-surface-800)_0_50%)] [background-size:6px_6px]";

/**
 * The room the strip takes, which is one vector component's.
 *
 * A column of value rows mixes colours, floats and vectors, and one width across them is what
 * keeps the readouts under each other.
 */
const STRIP_CLASS = "w-24";

interface ColorMarkProps {
  /** The row's `constantValue`, each channel 0 to 1. Null where the file writes none. */
  constant: ColorStop["rgba"] | null;
  /** The dynamics' stops, in the curve's own order. Empty where it has none. */
  stops: readonly ColorStop[];
}

/**
 * A `ValueColor`'s constant as a swatch, and its dynamics as a strip of its stops.
 *
 * A colour with no dynamics draws the swatch alone, and one whose file writes no constant draws
 * the strip alone.
 */
export function ColorMark({ constant, stops }: ColorMarkProps) {
  return (
    <span className="flex min-w-0 items-center gap-2">
      {constant !== null && <Swatch rgba={constant} />}
      {stops.length > 0 && <Strip stops={stops} />}
    </span>
  );
}

/** One colour over the checkerboard, and its bytes in hex on hover. */
export function Swatch({ rgba }: { rgba: ColorStop["rgba"] }) {
  return (
    <span
      title={colorHex(rgba)}
      data-ui="ColorMark:swatch"
      className={twMerge(
        "size-3.5 shrink-0 overflow-hidden rounded-sm border border-surface-veil-strong",
        CHECKER_CLASS,
      )}
    >
      <span className="block size-full" style={{ background: colorCss(rgba) }} />
    </span>
  );
}

/** The curve as one band, and its stops on hover. */
function Strip({ stops }: { stops: readonly ColorStop[] }) {
  const label = m.bin_gradient_label({ count: stops.length });
  return (
    <Tooltip content={<StopList stops={stops} />}>
      <span
        data-ui="ColorMark:strip"
        aria-label={label}
        className={twMerge(
          "h-3.5 shrink-0 overflow-hidden rounded-sm border border-surface-veil-strong",
          STRIP_CLASS,
          CHECKER_CLASS,
        )}
      >
        <span className="block size-full" style={{ background: gradientCss(stops) }} />
      </span>
    </Tooltip>
  );
}

/** Every stop, as the time it lands at and the colour there. */
function StopList({ stops }: { stops: readonly ColorStop[] }) {
  return (
    <div data-ui="ColorMark:card" className="flex flex-col gap-2 text-meta select-none">
      <span className="text-surface-400">{m.bin_gradient_label({ count: stops.length })}</span>
      <dl className="grid grid-cols-[auto_auto_1fr] items-center gap-x-3 gap-y-1">
        {stops.map((stop, at) => (
          <div key={at} className="col-span-3 grid grid-cols-subgrid items-center">
            <dt className="font-mono text-surface-400 tabular-nums">{stop.time.toFixed(3)}</dt>
            <dd className="flex">
              <Swatch rgba={stop.rgba} />
            </dd>
            <dd className="font-mono text-surface-200">{colorHex(stop.rgba)}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
