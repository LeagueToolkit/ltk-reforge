/**
 * One line of mono text cut in its middle to its box.
 *
 * Copied from LTK Manager's `src/modules/workshop/bin/shared/components/CutText.tsx`, with the
 * resize observer its `useResizeObserver` hook gives written in place.
 */
import { useEffect, useRef, useState } from "react";

import { twMerge } from "@/utils";

import { charsIn, cutText } from "../textCut";

/**
 * One line of mono text cut in its middle to the box it is given, the whole of it on hover.
 *
 * It grows into its flex line rather than sizing to its text, since a box measured off its own
 * cut text would narrow on every measure. A character's width is read off the text while it is
 * drawn whole, which a mono face makes one width for all of them.
 */
export function CutText({ text, className }: { text: string; className?: string }) {
  const [fit, setFit] = useState<number | null>(null);
  const box = useRef<HTMLSpanElement>(null);
  const drawn = useRef<HTMLSpanElement>(null);
  const ch = useRef(0);

  useEffect(() => {
    const element = box.current;
    if (element === null) return;
    const measure = () => {
      const line = drawn.current;
      if (line !== null && text.length > 0 && line.textContent === text) {
        ch.current = line.getBoundingClientRect().width / text.length;
      }
      setFit(charsIn(element.clientWidth, ch.current));
    };
    const observer = new ResizeObserver(measure);
    observer.observe(element);
    return () => observer.disconnect();
  }, [text]);
  const shown = cutText(text, fit);

  return (
    <span
      ref={box}
      title={shown === text ? undefined : text}
      className={twMerge("block min-w-0 flex-1 overflow-hidden whitespace-nowrap", className)}
    >
      <span ref={drawn}>{shown}</span>
    </span>
  );
}
