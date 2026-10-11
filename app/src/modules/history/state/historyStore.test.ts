import { beforeEach, describe, expect, it } from "vitest";

import { useHistoryStore } from "./historyStore";

const store = () => useHistoryStore.getState();

describe("the History page's state", () => {
  beforeEach(() => {
    store().start(-1);
    store().start(10);
  });

  it("starts at the index's head, and keeps what was asked while the head is the same", () => {
    expect(store().at).toBe(10);
    store().lookUp("data/a.bin", 4);
    store().start(10);
    expect(store().asked).toEqual({ query: "data/a.bin", at: 4 });
    store().start(11);
    expect(store()).toMatchObject({ head: 11, at: 11, asked: null, text: "" });
  });

  it("fills the object's bin from a lookup once, so coming back keeps the object", () => {
    store().lookUp("data/a.bin", 4);
    const asked = store().asked!;
    store().seed(asked, "data/a.bin", true);
    expect(store().bin).toBe("data/a.bin");
    store().openObject("data/a.bin", "Root");
    store().seed(asked, "data/a.bin", true);
    expect(store().object).toEqual({ bin: "data/a.bin", entry: "Root" });
  });

  it("starts the object and the comparison over at a new lookup", () => {
    store().lookUp("data/a.bin", 4);
    store().openObject("data/a.bin", "Root");
    store().setOther(2);
    store().lookUp("data/b.dds", 4);
    expect(store()).toMatchObject({ bin: "", entry: "", object: null, other: null });
    store().seed(store().asked!, "data/b.dds", false);
    expect(store().bin).toBe("");
  });

  it("shows the object typed, trimmed", () => {
    store().setBin(" data/a.bin ");
    store().setEntry(" Root ");
    store().showObject();
    expect(store().object).toEqual({ bin: "data/a.bin", entry: "Root" });
  });
});
