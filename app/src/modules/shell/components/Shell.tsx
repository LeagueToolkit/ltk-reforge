import { GearSixIcon, StackIcon, WrenchIcon } from "@phosphor-icons/react";
import { Link, type LinkProps } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { m } from "@/i18n";

function NavLink({
  to,
  exact,
  children,
}: {
  to: LinkProps["to"];
  exact?: boolean;
  children: ReactNode;
}) {
  return (
    <Link
      to={to}
      activeOptions={{ exact }}
      className="flex items-center gap-2 rounded-md px-3 py-2 text-sm text-surface-300 select-none hover:bg-surface-veil hover:text-surface-100 data-[status=active]:bg-surface-veil data-[status=active]:text-surface-50"
    >
      {children}
    </Link>
  );
}

interface ShellProps {
  /** A mark beside the inputs page's name, for inputs that call for it. */
  inputsMark?: ReactNode;
  children: ReactNode;
}

/** The app's frame: the pages on the left, the page beside them. */
export function Shell({ inputsMark, children }: ShellProps) {
  return (
    <div className="flex h-screen bg-surface-950 text-surface-100" data-ui="Shell">
      <nav
        aria-label={m.shell_nav_label()}
        className="flex w-52 shrink-0 flex-col gap-1 border-r border-surface-800 bg-surface-900 p-3"
        data-ui="Shell:nav"
      >
        <p className="px-3 py-2 font-display text-lg font-semibold text-surface-50 select-none">
          {m.common_app_name()}
        </p>
        <NavLink to="/" exact>
          <WrenchIcon weight="bold" className="h-4 w-4" />
          {m.shell_nav_repair_label()}
        </NavLink>
        <NavLink to="/inputs">
          <StackIcon weight="bold" className="h-4 w-4" />
          {m.shell_nav_inputs_label()}
          {inputsMark && <span className="ml-auto flex">{inputsMark}</span>}
        </NavLink>
        <NavLink to="/settings">
          <GearSixIcon weight="bold" className="h-4 w-4" />
          {m.shell_nav_settings_label()}
        </NavLink>
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto p-8" data-ui="Shell:main">
        {/* The width the pages are drawn at by the default window. A wider window buys
            margins, as LTK Manager's pages do: lines past it read worse, not better. */}
        <div className="mx-auto w-full max-w-4xl" data-ui="Shell:page">
          {children}
        </div>
      </main>
    </div>
  );
}
