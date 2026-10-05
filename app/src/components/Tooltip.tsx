import { Tooltip as BaseTooltip } from "@base-ui/react/tooltip";
import type { ReactElement, ReactNode } from "react";

export interface TooltipProps {
  content: ReactNode;
  /** The element hovered or focused. It takes the trigger's props and ref. */
  children: ReactElement<Record<string, unknown>>;
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
}

/** A note shown while an element is hovered or focused, LTK Manager's. */
export function Tooltip({ content, children, side = "top", align = "center" }: TooltipProps) {
  return (
    <BaseTooltip.Root>
      <BaseTooltip.Trigger render={children} />
      <BaseTooltip.Portal>
        <BaseTooltip.Positioner side={side} align={align} sideOffset={8} className="z-50">
          <BaseTooltip.Popup className="animate-fade-in rounded-md border border-surface-700 bg-surface-800 px-3 py-1.5 text-sm text-surface-100 shadow-lg data-[ending-style]:opacity-0 data-[starting-style]:opacity-0">
            <BaseTooltip.Arrow className="fill-surface-800 [&>path:first-child]:stroke-surface-700" />
            {content}
          </BaseTooltip.Popup>
        </BaseTooltip.Positioner>
      </BaseTooltip.Portal>
    </BaseTooltip.Root>
  );
}
