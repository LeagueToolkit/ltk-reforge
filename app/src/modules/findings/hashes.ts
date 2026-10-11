/**
 * The keys of a finding's fields whose hex is a hash a run's names may spell: a class, a field,
 * an object's entry, a hash, the class of a struct held (#ADR-0103: the report keeps hex).
 */
const SPELLED_KEYS: ReadonlySet<string> = new Set(["class", "field", "entry", "hash", "held"]);

/** The keys whose every field is such a hash: the field that holds a struct. */
const SPELLED_PARENTS: ReadonlySet<string> = new Set(["held_by"]);

/** Whether text is a hash as the report writes one: eight hex digits, or sixteen for a path. */
export function isHash(text: string): boolean {
  return /^(?:[0-9a-f]{8}|[0-9a-f]{16})$/i.test(text);
}

/** Whether a string under `key`, in a record under `parent`, is a hash to spell. */
export function spellsAt(text: string, key?: string, parent?: string): boolean {
  const named =
    (key !== undefined && SPELLED_KEYS.has(key)) ||
    (parent !== undefined && SPELLED_PARENTS.has(parent));
  return named && isHash(text);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** The hashes of a finding's fields to spell, each once, in the order found. */
export function hashesIn(value: unknown): string[] {
  const found = new Set<string>();
  const walk = (node: unknown, key?: string, parent?: string) => {
    if (typeof node === "string") {
      if (spellsAt(node, key, parent)) {
        found.add(node);
      }
    } else if (Array.isArray(node)) {
      // A list's items are under the list's key.
      for (const item of node) {
        walk(item, key, parent);
      }
    } else if (isRecord(node)) {
      for (const [field, child] of Object.entries(node)) {
        walk(child, field, key);
      }
    }
  };
  walk(value);
  return [...found];
}
