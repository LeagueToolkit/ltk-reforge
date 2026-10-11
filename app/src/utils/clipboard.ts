/**
 * Copies `text` to the clipboard.
 *
 * After LTK Manager's `src/hooks/useCopyToClipboard.ts`. Cut: the toast, which this app has
 * none of, so a failed copy reaches the console alone.
 */
export function copyText(text: string): void {
  navigator.clipboard.writeText(text).catch((error: unknown) => {
    console.error("the clipboard refused a copy", error);
  });
}
