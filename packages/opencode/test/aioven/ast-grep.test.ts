import { describe, expect, test } from "bun:test"
import { format } from "../../src/tool/ast-grep"

const match = (file: string, line: number, text: string, replacement?: string) => ({
  file,
  text,
  lines: `  ${text}  `,
  replacement,
  range: { start: { line } },
})

describe("ast_grep format", () => {
  test("lists matches as path:line: code relative to the root", () => {
    expect(format([match("/repo/src/a.ts", 4, "foo(1)")], "/repo", false)).toBe("src/a.ts:5: foo(1)")
  })

  test("shows rewrite previews", () => {
    expect(format([match("/repo/a.ts", 0, "foo(1)", "bar(1)")], "/repo", true)).toBe("a.ts:1: foo(1) → bar(1)")
  })

  test("caps output at 100 matches and handles none", () => {
    expect(format([], "/repo", false)).toBe("No matches")
    const many = Array.from({ length: 105 }, (_, i) => match("/repo/a.ts", i, "x"))
    const out = format(many, "/repo", false).split("\n")
    expect(out).toHaveLength(101)
    expect(out.at(-1)).toContain("5 more matches")
  })
})
