/**
 * A bin as LTK Manager's bin editor draws one, read-only: rows in a tree, a page at a time
 * (#ADR-0104).
 */
export { binOpenQuery, type BinRef } from "./api";
export { BinTree, type BinTreeProps } from "./components/BinTree";
export { CollapseAllButton } from "./components/CollapseAllButton";
export { CopyPathButton } from "./components/CopyPathButton";
export type { TreeReveal } from "./hooks/useReveal";
