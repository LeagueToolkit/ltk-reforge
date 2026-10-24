import { m } from "@/i18n";
import type { BatchRow, BatchUnit } from "@/lib/bindings.gen";

/**
 * Whether a unit is a premium skin pasted into its slot (#ADR-0107): its origin is a premium
 * skin on strong evidence from its mesh or its skeleton file.
 */
export function isPaste(unit: BatchUnit): boolean {
  const origin = unit.origin;
  return (
    origin.verdict === "premium" &&
    origin.strength === "strong" &&
    (origin.evidence ?? []).some((aspect) => aspect === "mesh" || aspect === "skeleton")
  );
}

/** Whether a scanned mod holds a unit that is a paste. */
export function hasPaste(row: BatchRow): boolean {
  return row.units.some(isPaste);
}

/** Whether a port would move the unit from the base to a premium slot. */
export function unitMovesToPremium(unit: BatchUnit): boolean {
  return unit.source === 0 && unit.chosen !== null && unit.chosen > 0;
}

/** Whether a port of the scanned mod would move a unit from the base to a premium slot. */
export function movesToPremium(row: BatchRow): boolean {
  return row.units.some(unitMovesToPremium);
}

/** A slot as the page spells it. */
export function slotText(slot: number | null): string {
  if (slot === null) {
    return m.manager_slot_game();
  }
  return slot === 0 ? m.manager_slot_base() : m.manager_slot_skin({ slot });
}

/** The aspects of the evidence, as a person reads them. */
export function aspectText(aspect: string): string {
  switch (aspect) {
    case "skin-name":
      return m.manager_aspect_skin_name();
    case "audio-tag":
      return m.manager_aspect_audio_tag();
    case "graph-closeness":
      return m.manager_aspect_graph();
    case "resolver-closeness":
      return m.manager_aspect_resolver();
    default:
      return aspect;
  }
}

/** A unit's origin in a few words: `paste of skin 20 (mesh, skeleton)`, `base`, `own work`. */
export function originText(unit: BatchUnit): string {
  const { verdict, slot, evidence } = unit.origin;
  const by = evidence && evidence.length > 0 ? ` (${evidence.map(aspectText).join(", ")})` : "";
  switch (verdict) {
    case "premium": {
      const skin = slotText(slot ?? null);
      const from = isPaste(unit)
        ? m.manager_origin_paste_of({ skin })
        : m.manager_origin_from({ skin });
      return `${from}${by}`;
    }
    case "base":
      return `${m.manager_slot_base()}${by}`;
    case "custom":
      return m.manager_origin_own();
    default:
      return m.manager_origin_unknown();
  }
}
