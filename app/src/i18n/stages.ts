import type { StageName } from "@/lib/bindings.gen";
import { m } from "@/paraglide/messages";

/** A stage's name for a person. */
export function stageLabel(stage: StageName): string {
  return m[`stage.${stage}.label`]();
}
