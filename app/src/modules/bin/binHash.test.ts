/** Copied from LTK Manager's `src/modules/workshop/bin/shared/utils/__tests__/binHash.test.ts`, cut to the value family's names. */
import { describe, expect, it } from "vitest";

import { fnv1a32, nameHash } from "./binHash";

/** Names the meta schema resolves, which a row's path and class carry as these hashes. */
const KNOWN: ReadonlyArray<readonly [name: string, hash: string]> = [
  ["SkinCharacterDataProperties", "0x9b67e9f6"],
  ["ValueColor", "0x074f91dd"],
  ["ValueFloat", "0x04300058"],
  ["ValueVector2", "0x69dc3449"],
  ["ValueVector3", "0x68dc32b6"],
  ["constantValue", "0xb4b427aa"],
  ["dynamics", "0xbc037de7"],
  ["times", "0x5d68eeb5"],
  ["values", "0x34474c3b"],
];

describe("fnv1a32", () => {
  it("hashes the empty string to the offset basis", () => {
    expect(fnv1a32("")).toBe(0x811c9dc5);
  });

  it("agrees with the game's hash on ltk_hash's own case", () => {
    expect(fnv1a32("test")).toBe(0xafd071e5);
  });

  it("reads a name without regard to case", () => {
    expect(fnv1a32("TEST")).toBe(fnv1a32("test"));
  });
});

describe("nameHash", () => {
  it.each(KNOWN)("hashes %s to %s", (name, hash) => {
    expect(nameHash(name)).toBe(hash);
  });
});
