import Markdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";

import { linkCodes, withoutHeading } from "../links";
import { RecordLink } from "./RecordLink";

/** The path prefix `linkCodes` writes a record's link with. */
const RECORD_PATH = "/records/";

/**
 * The elements of a record's text in the app's own classes, a `prose` of its tokens. A link to
 * a record opens its page; any other link reads as its text, since the records' other links
 * point into the repository, which the app does not hold.
 */
const COMPONENTS: Components = {
  h1: ({ node: _node, ...props }) => (
    <h2 className="mt-6 font-display text-lg font-semibold text-surface-50" {...props} />
  ),
  h2: ({ node: _node, ...props }) => (
    <h3 className="mt-6 font-display text-base font-semibold text-surface-50" {...props} />
  ),
  h3: ({ node: _node, ...props }) => (
    <h4 className="mt-4 text-sm font-semibold text-surface-100" {...props} />
  ),
  h4: ({ node: _node, ...props }) => (
    <h5 className="mt-4 text-sm font-medium text-surface-100" {...props} />
  ),
  p: ({ node: _node, ...props }) => <p className="mt-3 leading-relaxed" {...props} />,
  ul: ({ node: _node, ...props }) => (
    <ul className="mt-3 flex list-disc flex-col gap-1 pl-5 marker:text-surface-500" {...props} />
  ),
  ol: ({ node: _node, ...props }) => (
    <ol className="mt-3 flex list-decimal flex-col gap-1 pl-5 marker:text-surface-500" {...props} />
  ),
  li: ({ node: _node, ...props }) => (
    <li className="leading-relaxed [&>p:first-child]:mt-0 [&>ul]:mt-1" {...props} />
  ),
  strong: ({ node: _node, ...props }) => (
    <strong className="font-semibold text-surface-50" {...props} />
  ),
  blockquote: ({ node: _node, ...props }) => (
    <blockquote className="mt-3 border-l-2 border-surface-600 pl-4 text-surface-300" {...props} />
  ),
  hr: ({ node: _node, ...props }) => <hr className="my-6 border-surface-700" {...props} />,
  code: ({ node: _node, ...props }) => (
    <code
      className="rounded bg-surface-veil px-1 py-0.5 font-mono text-[0.85em] text-surface-100"
      {...props}
    />
  ),
  pre: ({ node: _node, ...props }) => (
    <pre
      className="mt-3 overflow-x-auto rounded-lg border border-surface-700/50 bg-surface-950/40 p-3 font-mono text-xs text-surface-200 [&_code]:bg-transparent [&_code]:p-0 [&_code]:text-xs"
      {...props}
    />
  ),
  table: ({ node: _node, ...props }) => (
    <div className="mt-3 overflow-x-auto">
      <table className="w-full border-collapse text-left text-xs" {...props} />
    </div>
  ),
  th: ({ node: _node, ...props }) => (
    <th
      className="border-b border-surface-600 px-2 py-1.5 font-semibold text-surface-100"
      {...props}
    />
  ),
  td: ({ node: _node, ...props }) => (
    <td className="border-b border-surface-700/50 px-2 py-1.5 align-top" {...props} />
  ),
  a: ({ href, children }) =>
    href?.startsWith(RECORD_PATH) ? (
      <RecordLink code={href.slice(RECORD_PATH.length)}>{children}</RecordLink>
    ) : (
      <span className="text-surface-100" title={href}>
        {children}
      </span>
    ),
  img: ({ alt }) => <span>{alt}</span>,
};

interface RecordMarkdownProps {
  /** The record's whole file. */
  markdown: string;
  /** Whether the bundle holds a code: a code it lacks stays text. */
  known: (code: string) => boolean;
}

/** A record's text rendered, its heading left to the page and each code it names a link. */
export function RecordMarkdown({ markdown, known }: RecordMarkdownProps) {
  return (
    <div className="text-sm text-surface-200 [&>*:first-child]:mt-0" data-ui="RecordMarkdown">
      <Markdown remarkPlugins={[remarkGfm]} components={COMPONENTS}>
        {linkCodes(withoutHeading(markdown), known)}
      </Markdown>
    </div>
  );
}
