/**
 * Copied from LTK Manager's `src/modules/workshop/bin/values/utils/__tests__/valueRows.test.ts`.
 * Cut: the probability tables, the ranges, the curve read of a scalar and the sparkline keys.
 */
import { describe, expect, it } from "vitest";

import type { BinRow, BinRows, BinValue } from "@/lib/bindings.gen";

import { nameHash } from "./binHash";
import {
  channels,
  colorHex,
  colorStops,
  constantRequests,
  dynamicsRequests,
  gradientCss,
  markText,
  markValue,
  placeTime,
  stopRequests,
  timeSpan,
  valueFamily,
  valueMarks,
} from "./valueRows";

const ENTRY = "0x2a1f3c7d";

/** The emitter's `birthColor`, whose wire path is the field's own hash. */
const COLOR_PATH = "0aaaaaaa";
const FLOAT_PATH = "0bbbbbbb";
const DYNAMICS_PATH = `${COLOR_PATH}.bc037de7`;

function row(path: string, value: BinValue, name = path): BinRow {
  return {
    entry: ENTRY,
    path,
    label: path,
    node: "property",
    name,
    unnamed: false,
    kind: null,
    value,
  };
}

function page(rows: BinRow[]): BinRows {
  return { rows, total: rows.length };
}

function struct(className: string, len: number): BinValue {
  return { type: "struct", classHash: nameHash(className), class: className, len };
}

function vec4(r: number, g: number, b: number, a: number): BinValue {
  return { type: "vector", values: [r, g, b, a] };
}

const colorRow = row(COLOR_PATH, struct("ValueColor", 2), "birthColor");
const floatRow = row(FLOAT_PATH, struct("ValueFloat", 2), "period");

/** The first level's answer: the colour's constant and its curve, and the float's constant. */
const CONSTANTS = new Map<string, BinRows>([
  [
    `${ENTRY}:${COLOR_PATH}`,
    page([
      row(`${COLOR_PATH}.b4b427aa`, vec4(1, 0.5, 0, 1)),
      row(DYNAMICS_PATH, struct("VfxAnimatedColorVariableData", 2)),
    ]),
  ],
  [
    `${ENTRY}:${FLOAT_PATH}`,
    page([
      row(`${FLOAT_PATH}.b4b427aa`, { type: "float", value: 2.5 }),
      row(`${FLOAT_PATH}.bc037de7`, { type: "null" }),
    ]),
  ],
]);

const DYNAMICS = new Map<string, BinRows>([
  [
    `${ENTRY}:${DYNAMICS_PATH}`,
    page([
      row(`${DYNAMICS_PATH}.5d68eeb5`, { type: "container", len: 2, itemKind: "f32" }),
      row(`${DYNAMICS_PATH}.34474c3b`, { type: "container", len: 2, itemKind: "vec4" }),
    ]),
  ],
]);

const STOPS = new Map<string, BinRows>([
  [
    `${ENTRY}:${DYNAMICS_PATH}.5d68eeb5`,
    page([
      row(`${DYNAMICS_PATH}.5d68eeb5[0]`, { type: "float", value: 0 }),
      row(`${DYNAMICS_PATH}.5d68eeb5[1]`, { type: "float", value: 2 }),
    ]),
  ],
  [
    `${ENTRY}:${DYNAMICS_PATH}.34474c3b`,
    page([
      row(`${DYNAMICS_PATH}.34474c3b[0]`, vec4(1, 0, 0, 1)),
      row(`${DYNAMICS_PATH}.34474c3b[1]`, vec4(0, 0, 1, 0)),
    ]),
  ],
]);

describe("valueFamily", () => {
  it("names the classes whose row draws its constant", () => {
    expect(valueFamily(struct("ValueColor", 2))).toBe("color");
    expect(valueFamily(struct("ValueFloat", 2))).toBe("scalar");
    expect(valueFamily(struct("ValueVector2", 2))).toBe("vector");
    expect(valueFamily(struct("ValueVector3", 2))).toBe("vector");
  });

  it("reads the three-channel colour class as a colour rather than as a vector", () => {
    expect(valueFamily(struct("ValueColorRgb", 2))).toBe("color");
  });

  it("names no other struct and no leaf", () => {
    expect(valueFamily(struct("VfxEmitterDefinitionData", 139))).toBeNull();
    expect(valueFamily({ type: "float", value: 1 })).toBeNull();
  });
});

describe("the three levels", () => {
  it("asks for every family row's own children first", () => {
    expect(constantRequests([colorRow, floatRow, row("0c", { type: "float", value: 1 })])).toEqual([
      { key: `${ENTRY}:${COLOR_PATH}`, rows: 2 },
      { key: `${ENTRY}:${FLOAT_PATH}`, rows: 2 },
    ]);
  });

  it("asks for the curve of a colour alone", () => {
    expect(dynamicsRequests([colorRow, floatRow], CONSTANTS)).toEqual([
      { key: `${ENTRY}:${DYNAMICS_PATH}`, rows: 2 },
    ]);
  });

  it("asks for nothing before the level above answers", () => {
    expect(dynamicsRequests([colorRow], new Map())).toEqual([]);
    expect(stopRequests(new Map())).toEqual([]);
  });

  it("asks for the two lists of every curve the level above answered", () => {
    expect(stopRequests(DYNAMICS)).toEqual([
      { key: `${ENTRY}:${DYNAMICS_PATH}.5d68eeb5`, rows: 2 },
      { key: `${ENTRY}:${DYNAMICS_PATH}.34474c3b`, rows: 2 },
    ]);
  });
});

describe("valueMarks", () => {
  const pages = { constants: CONSTANTS, dynamics: DYNAMICS, stops: STOPS };

  it("carries a colour's constant and its keys, paired by index", () => {
    const mark = valueMarks([colorRow], pages).get(`${ENTRY}:${COLOR_PATH}`);

    expect(mark?.family).toBe("color");
    expect(channels(mark?.constant ?? undefined)).toEqual([1, 0.5, 0, 1]);
    expect(mark?.keys).toEqual([
      { time: 0, values: [1, 0, 0, 1] },
      { time: 2, values: [0, 0, 1, 0] },
    ]);
    expect(mark?.curve).toBe(true);
  });

  it("carries a scalar's constant and no keys where it has no curve", () => {
    const mark = valueMarks([floatRow], pages).get(`${ENTRY}:${FLOAT_PATH}`);

    expect(mark?.family).toBe("scalar");
    expect(mark?.constant).toEqual({ type: "float", value: 2.5 });
    expect(mark?.keys).toEqual([]);
    expect(mark?.curve).toBe(false);
    expect(markValue(mark!)).toEqual({ type: "float", value: 2.5 });
  });

  it("carries a null constant while nothing has answered", () => {
    const empty = { constants: new Map(), dynamics: new Map(), stops: new Map() };

    expect(valueMarks([colorRow], empty).get(`${ENTRY}:${COLOR_PATH}`)).toEqual({
      family: "color",
      constant: null,
      keys: [],
      curve: false,
    });
  });
});

describe("colorStops and channels", () => {
  it("paints a four-channel key as a stop, and skips a key too short to be a colour", () => {
    expect(
      colorStops([
        { time: 0, values: [1, 0, 0, 1] },
        { time: 1, values: [0.5] },
      ]),
    ).toEqual([{ time: 0, rgba: [1, 0, 0, 1] }]);
  });

  it("paints a three-channel key opaque, which is what a ValueColorRgb key holds", () => {
    expect(colorStops([{ time: 0.5, values: [1, 0.5, 0] }])).toEqual([
      { time: 0.5, rgba: [1, 0.5, 0, 1] },
    ]);
  });

  it("takes a vec4 as it stands and a vec3 as an opaque colour", () => {
    expect(channels(vec4(1, 0.5, 0, 0.25))).toEqual([1, 0.5, 0, 0.25]);
    expect(channels({ type: "vector", values: [1, 0.5, 0] })).toEqual([1, 0.5, 0, 1]);
  });

  it("paints no colour out of a vector of another width, or one JSON could not carry", () => {
    expect(channels({ type: "vector", values: [1, 0] })).toBeNull();
    expect(channels({ type: "vector", values: [1, null, 0] })).toBeNull();
    expect(channels(null)).toBeNull();
  });
});

describe("timeSpan and placeTime", () => {
  it("is the particle's own life where every key falls inside it", () => {
    expect(timeSpan([0.25, 0.75])).toEqual({ first: 0, last: 1 });
    expect(placeTime(0.25, timeSpan([0.25, 0.75]))).toBeCloseTo(0.25);
  });

  it("widens to whichever end a key reaches past", () => {
    expect(timeSpan([-0.5, 0.5])).toEqual({ first: -0.5, last: 1 });
    expect(timeSpan([0.5, 4])).toEqual({ first: 0, last: 4 });
  });
});

describe("colorHex and gradientCss", () => {
  it("writes a colour as its bytes, clamped", () => {
    expect(colorHex([1, 0.5, 0, 1])).toBe("#FF8000FF");
    expect(colorHex([2, -1, 0, 1])).toBe("#FF0000FF");
  });

  it("places each stop at its own time in the window the stops span", () => {
    expect(
      gradientCss([
        { time: 1, rgba: [1, 0, 0, 1] },
        { time: 2, rgba: [0, 1, 0, 1] },
        { time: 5, rgba: [0, 0, 1, 1] },
      ]),
    ).toBe(
      "linear-gradient(to right, rgba(255, 0, 0, 1) 20.00%, rgba(0, 255, 0, 1) 40.00%, rgba(0, 0, 255, 1) 100.00%)",
    );
  });

  it("draws one stop as a band of its own colour, and nothing for no stops", () => {
    expect(gradientCss([{ time: 0, rgba: [1, 0, 0, 0.5] }])).toBe(
      "linear-gradient(to right, rgba(255, 0, 0, 0.5), rgba(255, 0, 0, 0.5))",
    );
    expect(gradientCss([])).toBe("");
  });
});

describe("markText", () => {
  it("copies a colour as its bytes and a scalar and a vector as they draw", () => {
    expect(
      markText({ family: "color", constant: vec4(1, 0.5, 0, 1), keys: [], curve: false }),
    ).toBe("#FF8000FF");
    expect(
      markText({
        family: "scalar",
        constant: { type: "float", value: 2.5 },
        keys: [],
        curve: false,
      }),
    ).toBe("2.5");
    expect(
      markText({
        family: "vector",
        constant: { type: "vector", values: [0, 1.5, 0] },
        keys: [],
        curve: false,
      }),
    ).toBe("0, 1.5, 0");
  });

  it("copies nothing before the read lands", () => {
    expect(markText(undefined)).toBeNull();
    expect(markText({ family: "color", constant: null, keys: [], curve: false })).toBeNull();
  });
});
