import { titleParts } from "../heads";

/** A record's title, its code spans in mono and its backticks left out. */
export function RecordTitle({ title }: { title: string }) {
  return (
    <>
      {titleParts(title).map((part, at) =>
        part.code ? (
          <code key={at} className="font-mono text-[0.9em]">
            {part.text}
          </code>
        ) : (
          part.text
        ),
      )}
    </>
  );
}
