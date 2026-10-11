import { Menu as BaseMenu } from "@base-ui/react/menu";
import type { ReactElement, ReactNode } from "react";

import { twMerge } from "@/utils";

/** One action of a menu. */
export interface MenuAction {
  label: ReactNode;
  icon?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
  /** Whether the action removes or discards something. */
  danger?: boolean;
}

/** A line between groups of actions. */
export interface MenuSeparator {
  separator: true;
}

export type MenuItem = MenuAction | MenuSeparator;

export interface MenuProps {
  /** The element that opens the menu, a `Button` or an `IconButton`. It takes the trigger's props and ref. */
  trigger: ReactElement<Record<string, unknown>>;
  items: MenuItem[];
  side?: "top" | "right" | "bottom" | "left";
  align?: "start" | "center" | "end";
}

/** A list of actions under a button: Base UI's menu, LTK Manager's look. */
export function Menu({ trigger, items, side = "bottom", align = "end" }: MenuProps) {
  return (
    <BaseMenu.Root>
      <BaseMenu.Trigger render={trigger} />
      <BaseMenu.Portal>
        <BaseMenu.Positioner side={side} align={align} sideOffset={6} className="z-50">
          <BaseMenu.Popup
            className="flex min-w-48 flex-col rounded-lg border border-surface-700 bg-surface-800 p-1 shadow-lg"
            data-ui="Menu"
          >
            {items.map((item, index) =>
              "separator" in item ? (
                <BaseMenu.Separator key={index} className="my-1 h-px bg-surface-700" />
              ) : (
                <BaseMenu.Item
                  key={index}
                  className={twMerge(
                    "flex cursor-pointer items-center gap-2 rounded-md px-3 py-1.5 text-sm outline-none select-none data-[disabled]:cursor-default data-[disabled]:text-surface-500 data-[highlighted]:bg-surface-700",
                    item.danger ? "text-danger-text" : "text-surface-100",
                  )}
                  disabled={item.disabled}
                  onClick={item.onSelect}
                >
                  {item.icon && (
                    <span className="inline-flex h-4 w-4 shrink-0 items-center justify-center">
                      {item.icon}
                    </span>
                  )}
                  {item.label}
                </BaseMenu.Item>
              ),
            )}
          </BaseMenu.Popup>
        </BaseMenu.Positioner>
      </BaseMenu.Portal>
    </BaseMenu.Root>
  );
}
