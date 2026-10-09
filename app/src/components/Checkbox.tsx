import { Checkbox as BaseCheckbox } from "@base-ui/react/checkbox";
import { CheckIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import { twMerge } from "@/utils";

export interface CheckboxProps {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** The text beside the box, which is also its accessible name. */
  label: ReactNode;
  className?: string;
}

/** A box ticked and unticked, with its label, LTK Manager's. */
export function Checkbox({ checked, onCheckedChange, label, className }: CheckboxProps) {
  return (
    <label
      className={twMerge(
        "flex cursor-pointer items-center gap-2 text-sm text-surface-200 select-none",
        className,
      )}
      data-ui="Checkbox"
    >
      <BaseCheckbox.Root
        checked={checked}
        onCheckedChange={(next) => onCheckedChange(next)}
        className={twMerge(
          "flex h-4 w-4 shrink-0 items-center justify-center rounded-sm border transition-colors",
          "border-surface-600 bg-surface-950/40 data-[checked]:border-accent-500 data-[checked]:bg-accent-500",
          "focus-visible:ring-2 focus-visible:ring-accent-500 focus-visible:ring-offset-2 focus-visible:ring-offset-surface-900 focus-visible:outline-none",
        )}
      >
        <BaseCheckbox.Indicator className="text-surface-900">
          <CheckIcon weight="bold" className="h-3 w-3" />
        </BaseCheckbox.Indicator>
      </BaseCheckbox.Root>
      {label}
    </label>
  );
}
