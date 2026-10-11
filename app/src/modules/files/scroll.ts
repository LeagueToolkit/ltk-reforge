/** A span along a box's height: where it starts and how tall it is. */
export interface Span {
  top: number;
  height: number;
}

/**
 * The scroll offset of a box that brings a row inside it into view by the least move, as a
 * scroll to the row's nearest edge does, but for that box alone: the page around stays where
 * it is, so the viewer's own scroll is the one that moves it.
 */
export function nearestScrollTop(box: Span, scrollTop: number, row: Span): number {
  const above = row.top - box.top;
  const below = row.top + row.height - (box.top + box.height);
  if (above < 0) {
    return scrollTop + above;
  }
  if (below > 0) {
    // A row taller than the box shows its top.
    return scrollTop + Math.min(below, above);
  }
  return scrollTop;
}

/** Scrolls the box a node scrolls in, and only that box, to show the node. */
export function showInScrollBox(node: HTMLElement): void {
  let box = node.parentElement;
  while (box && !/(auto|scroll)/.test(getComputedStyle(box).overflowY)) {
    box = box.parentElement;
  }
  if (!box) {
    return;
  }
  const at = box.getBoundingClientRect();
  const row = node.getBoundingClientRect();
  box.scrollTop = nearestScrollTop(
    { top: at.top + box.clientTop, height: box.clientHeight },
    box.scrollTop,
    { top: row.top, height: row.height },
  );
}
