import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import type { ReactNode } from "react";

import { twMerge } from "@/utils";

export interface TabItem<T extends string> {
  value: T;
  label: ReactNode;
  /** A small count beside the label, left out when zero. */
  count?: number;
}

export interface TabsProps<T extends string> {
  tabs: TabItem<T>[];
  value: T;
  onChange: (value: T) => void;
  /** The panels, each a `TabPanel` of one tab's value. */
  children: ReactNode;
  /** The list's accessible name. */
  "aria-label"?: string;
  className?: string;
}

/**
 * A row of tabs over one panel at a time: Base UI's tabs, with an underline that slides to the
 * active tab. The chosen tab is the caller's, so a page can switch it on its own events.
 */
export function Tabs<T extends string>({
  tabs,
  value,
  onChange,
  children,
  "aria-label": ariaLabel,
  className,
}: TabsProps<T>) {
  return (
    <BaseTabs.Root
      value={value}
      onValueChange={(next) => onChange(next as T)}
      className={twMerge("flex flex-col gap-6", className)}
      data-ui="Tabs"
    >
      <BaseTabs.List
        aria-label={ariaLabel}
        className="relative flex flex-wrap gap-1 border-b border-surface-700/50"
      >
        {tabs.map((tab) => (
          <BaseTabs.Tab
            key={tab.value}
            value={tab.value}
            className="relative flex h-9 cursor-pointer items-center gap-1.5 px-3 text-sm text-surface-400 select-none hover:text-surface-200 focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-accent-500 data-[active]:text-accent-300"
          >
            {tab.label}
            {tab.count != null && tab.count > 0 && (
              <span className="rounded-full bg-accent-500/15 px-1.5 text-xs text-accent-300 ring-1 ring-accent-500/35 ring-inset">
                {tab.count}
              </span>
            )}
          </BaseTabs.Tab>
        ))}
        <BaseTabs.Indicator
          className={twMerge(
            "absolute bottom-0 left-0 h-0.5 rounded-full bg-accent-500",
            "w-[var(--active-tab-width)] translate-x-[var(--active-tab-left)]",
            "transition-[transform,width] duration-200 ease-out",
          )}
        />
      </BaseTabs.List>
      {children}
    </BaseTabs.Root>
  );
}

export interface TabPanelProps<T extends string> {
  value: T;
  children: ReactNode;
  className?: string;
}

/**
 * One tab's panel, shown while its tab is active and kept mounted behind the others: what a
 * person opened in it is as they left it when they come back.
 */
export function TabPanel<T extends string>({ value, children, className }: TabPanelProps<T>) {
  return (
    <BaseTabs.Panel value={value} keepMounted className={twMerge("flex flex-col gap-6", className)}>
      {children}
    </BaseTabs.Panel>
  );
}
