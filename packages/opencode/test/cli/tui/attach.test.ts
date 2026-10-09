import { describe, expect, test } from "bun:test"

describe("tui attach", () => {
  test("starts the Ratatui client with --attach", async () => {
    const source = await Bun.file(new URL("../../../src/cli/cmd/attach.ts", import.meta.url)).text()

    expect(source).toContain('const forward = ["--attach", args.url]')
    expect(source).toContain("await launchRustTui(forward)")
  })
})
