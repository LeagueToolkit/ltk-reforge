import type { StageName } from "@/lib/bindings.gen";
import {
  "stage.build.label" as build,
  "stage.check.label" as check,
  "stage.decide.label" as decide,
  "stage.facts.label" as facts,
  "stage.fix.label" as fix,
  "stage.load.label" as load,
  "stage.plan.label" as plan,
  "stage.read.label" as read,
  "stage.write.label" as write,
} from "@/paraglide/messages";

// Named imports, so the linter checks each name against the catalog.
const LABELS: Record<StageName, () => string> = {
  read,
  load,
  fix,
  facts,
  decide,
  plan,
  build,
  check,
  write,
};

/** A stage's name for a person. */
export function stageLabel(stage: StageName): string {
  return LABELS[stage]();
}
