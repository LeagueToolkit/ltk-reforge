import { extendTailwindMerge } from "tailwind-merge";

/**
 * `twMerge`, taught the type scale of the theme.
 *
 * Stock tailwind-merge reads `text-*` against Tailwind's own sizes, so a tier of the theme's
 * falls in the text-colour group and a colour beside it deletes it:
 * `twMerge("text-meta", "text-surface-300")` gives `"text-surface-300"`.
 */
export const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [{ text: ["row", "meta", "fine", "code", "mono-row"] }],
    },
  },
});
