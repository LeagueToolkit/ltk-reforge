import { m } from "@/i18n";
import type { PackageFormat } from "@/lib/bindings.gen";

/** The forms a package is written in, the default first (#ADR-0105). */
export const PACKAGE_FORMATS: PackageFormat[] = ["fantome", "modpkg"];

/** The form a package is written in when none is chosen. */
export const DEFAULT_PACKAGE_FORMAT: PackageFormat = "fantome";

/** A format's label, which names the default as such. */
export function formatLabel(format: PackageFormat): string {
  return m[`format.${format}.label`]();
}
