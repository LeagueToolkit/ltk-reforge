/**
 * The text of the lines that fence the object asked for in a bin's text, as the backend writes
 * them (`ENTRY_FENCE`, on `FileView::Bin`): a control no object's text holds.
 */
export const ENTRY_FENCE = "\u001e";

/** A bin's text split at its fences: the object asked for, and the objects after it. */
export interface Fenced {
  /** The object asked for. Null when the text holds none. */
  asked: string | null;
  /** The other objects. */
  rest: string;
}

/** A bin's text split at the two lines that fence the object asked for, which comes first. */
export function fencedObject(text: string): Fenced {
  const fence = `${ENTRY_FENCE}\n`;
  const end = text.startsWith(fence) ? text.indexOf(fence, fence.length) : -1;
  if (end < 0) {
    return { asked: null, rest: text };
  }
  return { asked: text.slice(fence.length, end), rest: text.slice(end + fence.length) };
}
