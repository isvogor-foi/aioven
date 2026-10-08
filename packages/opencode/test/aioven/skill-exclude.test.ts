import { describe, expect, test } from "bun:test"
import { excluded } from "../../src/skill"

describe("skill exclusion", () => {
  test("built-in AIOven exclusions and config exclusions are skipped", () => {
    expect(excluded("morning")).toBe(true)
    expect(excluded("import-memory")).toBe(true)
    expect(excluded("pdf")).toBe(false)
    expect(excluded("pdf", ["pdf"])).toBe(true)
  })
})
