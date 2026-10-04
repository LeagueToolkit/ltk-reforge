import { match } from "ts-pattern";

import type { AppError } from "@/lib/bindings.gen";
import { m } from "@/paraglide/messages";
import { isAppError } from "@/utils";

/** The copy for one error: what went wrong, the remedy, and any text from outside the app. */
export interface ErrorCopy {
  title: string;
  description?: string;
  /** Text from outside the app, shown as data. */
  detail?: string;
}

/** The copy for a command's error, exhaustive over its `code` so a new variant fails `tsc`. */
export function describeError(error: AppError): ErrorCopy {
  return match(error)
    .with({ code: "SETTINGS_UNWRITABLE" }, ({ detail }) => ({
      title: m["error.SETTINGS_UNWRITABLE.title"](),
      detail,
    }))
    .with({ code: "NO_DATA_DIR" }, ({ detail }) => ({
      title: m["error.NO_DATA_DIR.title"](),
      detail,
    }))
    .with({ code: "NO_GAME" }, () => ({
      title: m["error.NO_GAME.title"](),
      description: m["error.NO_GAME.description"](),
    }))
    .with({ code: "GAME_UNREADABLE" }, ({ path, detail }) => ({
      title: m["error.GAME_UNREADABLE.title"]({ path }),
      detail,
    }))
    .with({ code: "NAMES_UNREADABLE" }, ({ detail }) => ({
      title: m["error.NAMES_UNREADABLE.title"](),
      detail,
    }))
    .with({ code: "UNKNOWN_FIXUP" }, ({ id }) => ({
      title: m["error.UNKNOWN_FIXUP.title"]({ id }),
    }))
    .with({ code: "UNKNOWN_CHECK" }, ({ id }) => ({
      title: m["error.UNKNOWN_CHECK.title"]({ id }),
    }))
    .with({ code: "BAD_FILTER" }, ({ filter, detail }) => ({
      title: m["error.BAD_FILTER.title"]({ filter }),
      detail,
    }))
    .with({ code: "NO_OUTPUT" }, () => ({
      title: m["error.NO_OUTPUT.title"](),
    }))
    .with({ code: "INTERNAL" }, ({ detail }) => ({
      title: m["error.INTERNAL.title"](),
      detail,
    }))
    .exhaustive();
}

/** The copy for anything thrown: a command's error, or a fault of the front end's own. */
export function describeFailure(failure: unknown): ErrorCopy {
  if (isAppError(failure)) {
    return describeError(failure);
  }
  return { title: m.error_unknown_title(), detail: String(failure) };
}
