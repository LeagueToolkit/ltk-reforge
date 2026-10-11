import { Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { twMerge } from "@/utils";

import { routeCode } from "../links";

interface RecordLinkProps {
  /** The record's code, with or without its `#`. */
  code: string;
  /** What the link reads; the code when absent. */
  children?: ReactNode;
  className?: string;
}

/**
 * A record's code as a link to its page. A click stays with the link, so a link inside a row
 * that opens on click, such as a finding's, does not open the row as well.
 */
export function RecordLink({ code, children, className }: RecordLinkProps) {
  return (
    <Link
      to="/records/$code"
      params={{ code: routeCode(code) }}
      onClick={(event) => event.stopPropagation()}
      className={twMerge(
        // A code breaks at its hyphen otherwise, half of it on each line.
        "font-mono whitespace-nowrap text-accent-300 underline-offset-2 hover:underline focus-visible:underline",
        className,
      )}
      data-ui="RecordLink"
    >
      {children ?? code}
    </Link>
  );
}
