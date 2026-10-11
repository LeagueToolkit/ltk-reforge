/**
 * A record's code as text names it, `#GAME-CHANGE-0005`: no digit or letter after its four
 * digits, so a longer number is not read as one.
 */
const CODE = /#(?:ADR|GAME-CHANGE|GAME-BEHAVIOR|MOD-ARTIFACT)-\d{4}(?![0-9A-Za-z])/g;

/** A fence that opens or closes a block of code: three or more backticks or tildes. */
const FENCE = /^ {0,3}(`{3,}|~{3,})/;

/** The route's part of a code: `GAME-CHANGE-0005` of `#GAME-CHANGE-0005`. */
export function routeCode(code: string): string {
  return code.replace(/^#/, "");
}

/** The path of a record's page. */
export function recordPath(code: string): string {
  return `/records/${routeCode(code)}`;
}

/** A record's text without its heading, which the page draws from the record's head. */
export function withoutHeading(markdown: string): string {
  return /^# /.test(markdown) ? markdown.slice(markdown.indexOf("\n") + 1) : markdown;
}

/** Prose with each code it names that `known` holds as a Markdown link to the record. */
function linkProse(prose: string, known: (code: string) => boolean): string {
  return prose.replace(CODE, (code) => (known(code) ? `[${code}](${recordPath(code)})` : code));
}

/**
 * A paragraph with its codes linked outside its code spans. A span opens at a run of backticks
 * and closes at the next run of as many; a run with no match is text (CommonMark).
 */
function linkParagraph(paragraph: string, known: (code: string) => boolean): string {
  let out = "";
  let at = 0;
  const runs = /`+/g;
  for (let open = runs.exec(paragraph); open; open = runs.exec(paragraph)) {
    const ticks = open[0];
    let close = runs.exec(paragraph);
    while (close && close[0].length !== ticks.length) {
      close = runs.exec(paragraph);
    }
    if (!close) {
      // Unmatched: the text after this run is prose, scanned again for a shorter run.
      runs.lastIndex = open.index + ticks.length;
      continue;
    }
    out += linkProse(paragraph.slice(at, open.index), known);
    out += paragraph.slice(open.index, close.index + ticks.length);
    at = close.index + ticks.length;
  }
  return out + linkProse(paragraph.slice(at), known);
}

/**
 * Markdown with each record's code it names a link to the record, when `known` holds the
 * code. A code in a block of code or a code span stays as written.
 */
export function linkCodes(markdown: string, known: (code: string) => boolean): string {
  const out: string[] = [];
  // Lines of prose since the last fence or blank line: a code span does not cross either.
  let paragraph: string[] = [];
  let fence: string | null = null;
  const flush = () => {
    if (paragraph.length > 0) {
      out.push(linkParagraph(paragraph.join("\n"), known));
      paragraph = [];
    }
  };
  for (const line of markdown.split("\n")) {
    const marker = FENCE.exec(line)?.[1];
    if (fence !== null) {
      out.push(line);
      if (marker && marker[0] === fence[0] && marker.length >= fence.length) {
        fence = null;
      }
    } else if (marker) {
      flush();
      out.push(line);
      fence = marker;
    } else if (line.trim() === "") {
      flush();
      out.push(line);
    } else {
      paragraph.push(line);
    }
  }
  flush();
  return out.join("\n");
}
