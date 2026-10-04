import { defineConfig } from "@inlang/paraglide-js";

// Tracked with `git add -f`: the inlang SDK rewrites this folder's .gitignore to admit only
// settings.json.
export default defineConfig({
  outdir: "./src/paraglide",
  strategy: ["baseLocale"],
});
