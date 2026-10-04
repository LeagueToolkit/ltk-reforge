import { describe, expect, it } from "vitest";

import { asksFirst, type Missing } from "./InputsGate";

const nothing: Missing = { game: false, index: false, hashes: false, schema: false };

describe("asksFirst", () => {
  it("runs at once when every input is there", () => {
    expect(asksFirst(nothing, {})).toBe(false);
  });

  it("asks before a run with no history index", () => {
    expect(asksFirst({ ...nothing, index: true }, {})).toBe(true);
  });

  it("does not ask again once told not to", () => {
    expect(asksFirst({ ...nothing, index: true }, { "prompts.runWithLess": true })).toBe(false);
  });

  it("always asks when there is no game, whatever the setting", () => {
    expect(asksFirst({ ...nothing, game: true }, { "prompts.runWithLess": true })).toBe(true);
  });
});
