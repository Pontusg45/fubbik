import { describe, expect, it } from "vitest";
import { getTableColumns } from "drizzle-orm";
import { chunk, chunkConnection } from "./chunk";

describe("chunk table", () => {
  it("has expected columns", () => {
    const columns = getTableColumns(chunk);
    expect(columns).toHaveProperty("id");
    expect(columns).toHaveProperty("title");
    expect(columns).toHaveProperty("content");
    expect(columns).toHaveProperty("type");
    expect(columns).toHaveProperty("tags");
    expect(columns).toHaveProperty("userId");
    expect(columns).toHaveProperty("createdAt");
    expect(columns).toHaveProperty("updatedAt");
  });
});

describe("chunkConnection table", () => {
  it("has expected columns", () => {
    const columns = getTableColumns(chunkConnection);
    expect(columns).toHaveProperty("id");
    expect(columns).toHaveProperty("sourceId");
    expect(columns).toHaveProperty("targetId");
    expect(columns).toHaveProperty("relation");
    expect(columns).toHaveProperty("createdAt");
  });
});
