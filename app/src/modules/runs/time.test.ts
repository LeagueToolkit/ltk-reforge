import { describe, expect, it } from "vitest";

import { ago, localTime, storeTime } from "./time";

// Local times, so that the calendar days are this machine's whatever its zone.
const now = new Date(2026, 9, 11, 15, 30, 0);
const at = (days: number, hours: number, minutes: number) =>
  new Date(2026, 9, 11 - days, hours, minutes, 0);

describe("ago", () => {
  it("says now within a minute", () => {
    expect(ago(new Date(now.getTime() - 20_000), now)).toEqual({ unit: "now" });
  });

  it("counts minutes within the hour", () => {
    expect(ago(at(0, 15, 27), now)).toEqual({ unit: "minutes", count: 3 });
  });

  it("counts hours within the day", () => {
    expect(ago(at(0, 9, 0), now)).toEqual({ unit: "hours", count: 6 });
  });

  it("says yesterday for the local day before, however few hours ago", () => {
    expect(ago(at(1, 22, 0), new Date(2026, 9, 11, 1, 30))).toEqual({ unit: "yesterday" });
    expect(ago(at(1, 8, 0), now)).toEqual({ unit: "yesterday" });
  });

  it("counts days within a week", () => {
    expect(ago(at(3, 12, 0), now)).toEqual({ unit: "days", count: 3 });
    expect(ago(at(6, 12, 0), now)).toEqual({ unit: "days", count: 6 });
  });

  it("gives the date beyond a week", () => {
    expect(ago(at(7, 12, 0), now)).toEqual({ unit: "date", date: "2026-10-04" });
  });
});

describe("storeTime", () => {
  it("reads the store's time as UTC", () => {
    expect(storeTime("2026-10-11 13:05:09").toISOString()).toBe("2026-10-11T13:05:09.000Z");
  });

  it("writes the full time by the local clock", () => {
    expect(localTime(new Date(2026, 0, 2, 3, 4, 5))).toBe("2026-01-02 03:04");
  });
});
