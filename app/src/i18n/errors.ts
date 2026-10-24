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
    .with({ code: "BAD_RULE" }, ({ id, detail }) => ({
      title: m["error.BAD_RULE.title"]({ id }),
      detail,
    }))
    .with({ code: "SELECTION_UNREADABLE" }, ({ path, detail }) => ({
      title: m["error.SELECTION_UNREADABLE.title"]({ path }),
      detail,
    }))
    .with({ code: "BAD_POLICY" }, ({ detail }) => ({
      title: m["error.BAD_POLICY.title"](),
      detail,
    }))
    .with({ code: "BAD_SETTING" }, ({ detail }) => ({
      title: m["error.BAD_SETTING.title"](),
      detail,
    }))
    .with({ code: "NO_OUTPUT" }, () => ({
      title: m["error.NO_OUTPUT.title"](),
    }))
    .with({ code: "INDEX_FETCH" }, ({ detail }) => ({
      title: m["error.INDEX_FETCH.title"](),
      detail,
    }))
    .with({ code: "INDEX_DIGEST" }, ({ file }) => ({
      title: m["error.INDEX_DIGEST.title"]({ file }),
      description: m["error.INDEX_DIGEST.description"](),
    }))
    .with({ code: "INDEX_UNREADABLE" }, ({ detail }) => ({
      title: m["error.INDEX_UNREADABLE.title"](),
      detail,
    }))
    .with({ code: "INDEX_UNWRITABLE" }, ({ detail }) => ({
      title: m["error.INDEX_UNWRITABLE.title"](),
      detail,
    }))
    .with({ code: "INDEX_BUSY" }, () => ({ title: m["error.INDEX_BUSY.title"]() }))
    .with({ code: "NO_INDEX_RELEASE" }, () => ({ title: m["error.NO_INDEX_RELEASE.title"]() }))
    .with({ code: "NO_INDEX" }, () => ({ title: m["error.NO_INDEX.title"]() }))
    .with({ code: "HISTORY_OBJECT_UNREADABLE" }, ({ bin, entry, detail }) => ({
      title: m["error.HISTORY_OBJECT_UNREADABLE.title"](),
      detail: `${bin} › ${entry}: ${detail}`,
    }))
    .with({ code: "CENSUS_FETCH" }, ({ detail }) => ({
      title: m["error.CENSUS_FETCH.title"](),
      detail,
    }))
    .with({ code: "INDEX_FOREIGN" }, ({ head }) => ({
      title: m["error.INDEX_FOREIGN.title"](),
      description: m["error.INDEX_FOREIGN.description"]({ head }),
    }))
    .with({ code: "INDEX_APPEND" }, ({ detail }) => ({
      title: m["error.INDEX_APPEND.title"](),
      detail,
    }))
    .with({ code: "NO_ARCHIVE" }, () => ({ title: m["error.NO_ARCHIVE.title"]() }))
    .with({ code: "CENSUS_DOWNLOAD" }, ({ detail }) => ({
      title: m["error.CENSUS_DOWNLOAD.title"](),
      description: m["error.CENSUS_DOWNLOAD.description"](),
      detail,
    }))
    .with({ code: "ARCHIVE_UNREADABLE" }, ({ detail }) => ({
      title: m["error.ARCHIVE_UNREADABLE.title"](),
      detail,
    }))
    .with({ code: "CENSUS_UNWRITABLE" }, ({ detail }) => ({
      title: m["error.CENSUS_UNWRITABLE.title"](),
      detail,
    }))
    .with({ code: "HASHES_FETCH" }, ({ detail }) => ({
      title: m["error.HASHES_FETCH.title"](),
      detail,
    }))
    .with({ code: "HASHES_BUSY" }, () => ({
      title: m["error.HASHES_BUSY.title"](),
      description: m["error.HASHES_BUSY.description"](),
    }))
    .with({ code: "SCHEMA_FETCH" }, ({ detail }) => ({
      title: m["error.SCHEMA_FETCH.title"](),
      detail,
    }))
    .with({ code: "SCHEMA_UNUSABLE" }, ({ detail }) => ({
      title: m["error.SCHEMA_UNUSABLE.title"](),
      description: m["error.SCHEMA_UNUSABLE.description"](),
      detail,
    }))
    .with({ code: "SCHEMA_UNWRITABLE" }, ({ detail }) => ({
      title: m["error.SCHEMA_UNWRITABLE.title"](),
      detail,
    }))
    .with({ code: "SCHEMA_BUSY" }, () => ({ title: m["error.SCHEMA_BUSY.title"]() }))
    .with({ code: "RUN_BUSY" }, () => ({
      title: m["error.RUN_BUSY.title"](),
      description: m["error.RUN_BUSY.description"](),
    }))
    .with({ code: "STORE_FAILED" }, ({ detail }) => ({
      title: m["error.STORE_FAILED.title"](),
      detail,
    }))
    .with({ code: "STORE_OUTDATED" }, () => ({
      title: m["error.STORE_OUTDATED.title"](),
      description: m["error.STORE_OUTDATED.description"](),
    }))
    .with({ code: "NO_FILES_AT_STAGE" }, ({ stage }) => ({
      title: m["error.NO_FILES_AT_STAGE.title"]({ stage: m[`stage.${stage}.label`]() }),
    }))
    .with({ code: "BLOB_MISSING" }, ({ id }) => ({
      title: m["error.BLOB_MISSING.title"](),
      description: m["error.BLOB_MISSING.description"](),
      detail: id,
    }))
    .with({ code: "BIN_UNREADABLE" }, ({ detail }) => ({
      title: m["error.BIN_UNREADABLE.title"](),
      detail,
    }))
    .with({ code: "BIN_NODE_NOT_FOUND" }, ({ address }) => ({
      title: m["error.BIN_NODE_NOT_FOUND.title"](),
      detail: address,
    }))
    .with({ code: "BIN_READ_TOO_WIDE" }, ({ rows, cap }) => ({
      title: m["error.BIN_READ_TOO_WIDE.title"]({ rows, cap }),
    }))
    .with({ code: "NO_PACKAGE_HELD" }, () => ({
      title: m["error.NO_PACKAGE_HELD.title"](),
      description: m["error.NO_PACKAGE_HELD.description"](),
    }))
    .with({ code: "NOT_RESUMED" }, ({ detail }) => ({
      title: m["error.NOT_RESUMED.title"](),
      description: m["error.NOT_RESUMED.description"](),
      detail,
    }))
    .with({ code: "NO_PROJECT_OF" }, () => ({
      title: m["error.NO_PROJECT_OF.title"](),
      description: m["error.NO_PROJECT_OF.description"](),
    }))
    .with({ code: "NO_EVIDENCE" }, ({ path }) => ({
      title: m["error.NO_EVIDENCE.title"](),
      description: m["error.NO_EVIDENCE.description"](),
      detail: path,
    }))
    .with({ code: "PROJECT_FORMAT" }, ({ found }) => ({
      title: m["error.PROJECT_FORMAT.title"]({ found: String(found) }),
      description: m["error.PROJECT_FORMAT.description"](),
    }))
    .with({ code: "PROJECT_MALFORMED" }, ({ detail }) => ({
      title: m["error.PROJECT_MALFORMED.title"](),
      detail,
    }))
    .with({ code: "NOT_REMOVED" }, ({ path, detail }) => ({
      title: m["error.NOT_REMOVED.title"]({ path }),
      description: m["error.NOT_REMOVED.description"](),
      detail,
    }))
    .with({ code: "MOD_UNREADABLE" }, ({ path, detail }) => ({
      title: m["error.MOD_UNREADABLE.title"](),
      detail: `${path}: ${detail}`,
    }))
    .with({ code: "NOT_SHOWN" }, ({ path, detail }) => ({
      title: m["error.NOT_SHOWN.title"](),
      detail: path ? `${path}: ${detail}` : detail,
    }))
    .with({ code: "FOLDER_UNREADABLE" }, ({ path, detail }) => ({
      title: m["error.FOLDER_UNREADABLE.title"](),
      detail: `${path}: ${detail}`,
    }))
    .with({ code: "MANAGER_ABSENT" }, ({ path }) => ({
      title: m["error.MANAGER_ABSENT.title"](),
      detail: path,
    }))
    .with({ code: "CANCELLED" }, () => ({ title: m["error.CANCELLED.title"]() }))
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
