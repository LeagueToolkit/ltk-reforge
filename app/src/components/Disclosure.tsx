import { Collapsible } from "@base-ui/react/collapsible";
import { CaretRightIcon } from "@phosphor-icons/react";
import type { ReactNode } from "react";

import { twMerge } from "@/utils";

export interface DisclosureProps {
  /** The row that opens and closes the panel. */
  summary: ReactNode;
  /** The panel, rendered only while open. */
  children: ReactNode;
  defaultOpen?: boolean;
  className?: string;
  summaryClassName?: string;
  panelClassName?: string;
}

/**
 * A row that opens to what it summarises: Base UI's collapsible, with a caret that turns as it
 * opens. Inside the summary, `group-data-[panel-open]:` styles the open state.
 */
export function Disclosure({
  summary,
  children,
  defaultOpen,
  className,
  summaryClassName,
  panelClassName,
}: DisclosureProps) {
  return (
    <Collapsible.Root defaultOpen={defaultOpen} className={className}>
      <Collapsible.Trigger
        className={twMerge(
          "group flex w-full cursor-pointer items-start gap-2 rounded-md text-left",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-500",
          summaryClassName,
        )}
      >
        <CaretRightIcon
          weight="bold"
          className="mt-1 h-3 w-3 shrink-0 text-surface-400 transition-transform group-data-[panel-open]:rotate-90"
        />
        <span className="min-w-0 flex-1">{summary}</span>
      </Collapsible.Trigger>
      <Collapsible.Panel className={panelClassName}>{children}</Collapsible.Panel>
    </Collapsible.Root>
  );
}
