import { describe, expect, it } from "vitest";

import { linkCodes, routeCode, withoutHeading } from "./links";

const every = () => true;

describe("linkCodes", () => {
  it("links a code in prose to its record", () => {
    expect(linkCodes("As #ADR-0046 says, the app", every)).toBe(
      "As [#ADR-0046](/records/ADR-0046) says, the app",
    );
  });

  it("links a code at the end of a sentence without the period", () => {
    expect(linkCodes("It rests on #GAME-CHANGE-0005.", every)).toBe(
      "It rests on [#GAME-CHANGE-0005](/records/GAME-CHANGE-0005).",
    );
  });

  it("leaves a code inside a backtick span as written", () => {
    const text = "Cite it as `#MOD-ARTIFACT-0001`, then #MOD-ARTIFACT-0002";
    expect(linkCodes(text, every)).toBe(
      "Cite it as `#MOD-ARTIFACT-0001`, then [#MOD-ARTIFACT-0002](/records/MOD-ARTIFACT-0002)",
    );
  });

  it("leaves a code inside a span that wraps onto the next line as written", () => {
    const text = "the line `Superseded\nby #ADR-0044` and #ADR-0044";
    expect(linkCodes(text, every)).toBe(
      "the line `Superseded\nby #ADR-0044` and [#ADR-0044](/records/ADR-0044)",
    );
  });

  it("leaves a code inside a fenced block as written", () => {
    const text = "```markdown\n- **Status:** Superseded by #ADR-0001\n```\n#ADR-0001";
    expect(linkCodes(text, every)).toBe(
      "```markdown\n- **Status:** Superseded by #ADR-0001\n```\n[#ADR-0001](/records/ADR-0001)",
    );
  });

  it("reads a lone backtick as text", () => {
    expect(linkCodes("a ` mark and #ADR-0002", every)).toBe(
      "a ` mark and [#ADR-0002](/records/ADR-0002)",
    );
  });

  it("leaves a code the bundle lacks, and a longer number, as text", () => {
    const known = (code: string) => code === "#ADR-0001";
    expect(linkCodes("#ADR-0001 #ADR-0002 #ADR-00012", known)).toBe(
      "[#ADR-0001](/records/ADR-0001) #ADR-0002 #ADR-00012",
    );
  });
});

describe("routeCode", () => {
  it("drops the hash", () => {
    expect(routeCode("#GAME-BEHAVIOR-0003")).toBe("GAME-BEHAVIOR-0003");
    expect(routeCode("GAME-BEHAVIOR-0003")).toBe("GAME-BEHAVIOR-0003");
  });
});

describe("withoutHeading", () => {
  it("drops the record's heading line", () => {
    expect(withoutHeading("# ADR-0001: A title\n\n- **Status:** Proposed\n")).toBe(
      "\n- **Status:** Proposed\n",
    );
  });
});
