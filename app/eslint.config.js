// LTK Manager's rules, to the extent this app has the code they govern.
import js from "@eslint/js";
import pluginQuery from "@tanstack/eslint-plugin-query";
import eslintConfigPrettier from "eslint-config-prettier/flat";
import i18next from "eslint-plugin-i18next";
import reactPlugin from "eslint-plugin-react";
import reactHooksPlugin from "eslint-plugin-react-hooks";
import simpleImportSort from "eslint-plugin-simple-import-sort";
import globals from "globals";
import tseslint from "typescript-eslint";

/** Output no rule can ask an author to change. */
const GENERATED = ["src/lib/bindings.gen.ts", "src/routeTree.gen.ts"];

/* The stock merger files a type tier of ours under text colour and drops it beside a colour.
   `@/utils` exports the one that knows the tiers. */
const TAILWIND_MERGE = {
  group: ["tailwind-merge"],
  message: "Merge classes with `twMerge` from `@/utils`.",
};

const RESTRICTED = [
  {
    group: ["@/components/*"],
    message: "Import a component through the barrel, `@/components`.",
  },
  {
    group: ["@base-ui/react/*", "@base-ui/react"],
    message: "Reach Base UI through its wrapper in `src/components`.",
  },
  {
    group: ["@/modules/*/**"],
    message: "Import a module through its barrel, `@/modules/<name>`.",
  },
  TAILWIND_MERGE,
];

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    plugins: {
      react: reactPlugin,
      "react-hooks": reactHooksPlugin,
      "simple-import-sort": simpleImportSort,
    },
    languageOptions: {
      globals: { ...globals.browser, ...globals.es2020 },
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    settings: { react: { version: "detect" } },
    rules: {
      ...reactPlugin.configs.recommended.rules,
      ...reactPlugin.configs["jsx-runtime"].rules,
      ...reactHooksPlugin.configs.recommended.rules,
      "@typescript-eslint/no-unused-vars": [
        "warn",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/no-explicit-any": "warn",
      "react/prop-types": "off",
      // Import order is oxfmt's `sortImports`.
      "simple-import-sort/exports": "error",
    },
  },
  ...pluginQuery.configs["flat/recommended-strict"],
  {
    files: ["src/**/*.{ts,tsx}"],
    ignores: GENERATED,
    rules: {
      "max-lines": ["warn", { max: 400, skipBlankLines: true, skipComments: true }],
    },
  },
  {
    // Every sentence a person reads is in `messages/`.
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/**/*.test.{ts,tsx}", ...GENERATED],
    plugins: { i18next },
    languageOptions: {
      parserOptions: {
        // Type information lets the rule skip a literal whose type is a string union.
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "i18next/no-literal-string": [
        "warn",
        {
          mode: "all",
          "jsx-attributes": {
            exclude: [
              ".*[cC]lassName",
              "data-ui",
              "to",
              "href",
              "id",
              "name",
              "type",
              "role",
              "variant",
              "size",
              "weight",
              "key",
            ],
          },
          callees: {
            exclude: ["invoke", "listen", "emit", "navigate", "console\\..*", "getElementById"],
          },
          "object-properties": {
            exclude: ["to", "search", "key", "id", "className", "data-ui"],
          },
          words: {
            exclude: [
              // Ids, paths and class tokens: copy has a capital or a space.
              "^[a-z0-9_./:-]+$",
              // Punctuation around a value the code interpolates.
              "^[\\s(){}\\[\\]<>,.:;/|·–—-]+$",
            ],
          },
        },
      ],
    },
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/**/*.test.{ts,tsx}", ...GENERATED],
    rules: {
      "no-restricted-imports": ["warn", { patterns: RESTRICTED }],
    },
  },
  {
    // A module reaches its own insides by relative paths, and the wrappers reach Base UI.
    files: ["src/components/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": ["warn", { patterns: [TAILWIND_MERGE] }],
    },
  },
  {
    files: ["src/utils/twMerge.ts"],
    rules: { "no-restricted-imports": "off" },
  },
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: { globals: { ...globals.node } },
  },
  {
    ignores: ["**/dist/", "**/node_modules/", "src/paraglide/", ...GENERATED],
  },
  eslintConfigPrettier,
);
