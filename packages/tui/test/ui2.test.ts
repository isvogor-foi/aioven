import { describe, expect, test } from "bun:test"
import type { ToolPart } from "@opencode-ai/sdk/v2"
import { assign, infer, parsePlan, status } from "../src/routes/session/ui2/plan-model"
import { compact } from "../src/routes/session/ui2/compact"

const PLAN = `# Retry plan

## Context
whatever

## Architecture
### Components
- **RetryPolicy** (new) — paths: src/retry.ts — backoff rules
- **FetchClient** (reuse) — paths: \`src/client.ts\`, src/http/ — calls retry
* **ClientTests** (new) - paths: test/client.test.ts - tests
- not a component line

### Interfaces
- **Ignored** (new) — paths: x.ts — outside components
`

describe("parsePlan", () => {
  test("reads components in the U7 format only from the Components section", () => {
    expect(parsePlan(PLAN)).toEqual([
      { name: "RetryPolicy", kind: "new", paths: ["src/retry.ts"] },
      { name: "FetchClient", kind: "reuse", paths: ["src/client.ts", "src/http/"] },
      { name: "ClientTests", kind: "new", paths: ["test/client.test.ts"] },
    ])
  })

  test("no components section gives an empty list", () => {
    expect(parsePlan("# plan\n- **X** (new) — paths: a")).toEqual([])
  })
})

describe("assign", () => {
  test("matches exact files and directory prefixes, rest is other", () => {
    const comps = parsePlan(PLAN)
    const file = (f: string) => ({ file: f, additions: 1, deletions: 0 })
    const { byComponent, other } = assign(comps, [file("src/retry.ts"), file("src/http/agent.ts"), file("src/httpx.ts"), file("bun.lock")])
    expect(byComponent.get("RetryPolicy")!.map((f) => f.file)).toEqual(["src/retry.ts"])
    expect(byComponent.get("FetchClient")!.map((f) => f.file)).toEqual(["src/http/agent.ts"])
    expect(byComponent.get("ClientTests")).toEqual([])
    expect(other.map((f) => f.file)).toEqual(["src/httpx.ts", "bun.lock"])
  })
})

describe("status", () => {
  const comp = { name: "RetryPolicy", kind: "new" as const, paths: [] }
  test("derived from todos that name the component", () => {
    expect(status(comp, [])).toBe("pending")
    expect(status(comp, [{ content: "Build retrypolicy", status: "in_progress" }])).toBe("in_progress")
    expect(status(comp, [{ content: "RetryPolicy tests", status: "completed" }])).toBe("completed")
    expect(status(comp, [{ content: "RetryPolicy", status: "completed" }, { content: "RetryPolicy 2", status: "pending" }])).toBe("pending")
  })
})

describe("infer", () => {
  test("groups by package or top-level folder", () => {
    const file = (f: string) => ({ file: f, additions: 0, deletions: 0 })
    expect(infer([file("packages/tui/src/a.ts"), file("packages/tui/b.ts"), file("src/x.ts"), file("README.md")])).toEqual([
      { name: "packages/tui", kind: "reuse", paths: ["packages/tui"] },
      { name: "src", kind: "reuse", paths: ["src"] },
      { name: "project root", kind: "reuse", paths: [] },
    ])
  })
})

describe("compact", () => {
  const part = (tool: string, state: Record<string, unknown>) => ({ type: "tool", tool, state }) as unknown as ToolPart
  const done = (input: Record<string, unknown>, metadata: Record<string, unknown> = {}) => ({
    status: "completed",
    input,
    metadata,
    output: "",
    title: "",
    time: { start: 0, end: 12_000 },
  })

  test("summarises tools in one line without code", () => {
    expect(compact(part("edit", done({ filePath: "/r/src/a.ts" }, { filediff: { additions: 4, deletions: 1 } })), (f) => f.replace("/r/", ""))).toEqual({
      icon: "✎",
      text: "edited src/a.ts +4 −1",
    })
    expect(compact(part("bash", done({ command: "bun test\nmore" })))).toEqual({ icon: "⏵", text: "bun test (12s)" })
    expect(compact(part("task", done({ subagent_type: "explore", description: "find calls" })))).toEqual({ icon: "⇢", text: "explore: find calls" })
    expect(compact(part("apply_patch", done({}, { files: [{ relativePath: "a.ts", additions: 2, deletions: 0 }] }))).text).toBe("edited a.ts +2 −0")
  })

  test("running and failed states change the icon", () => {
    expect(compact(part("bash", { status: "running", input: { command: "ls" }, time: { start: 0 } })).icon).toBe("⏳")
    expect(compact(part("read", { status: "error", input: { filePath: "x" }, error: "no" })).icon).toBe("✗")
  })
})

import { fitTabs } from "../src/routes/session/ui2/tabs"

describe("fitTabs", () => {
  const tabs = [
    { index: 0, name: "realestate-orchestrator", status: "⏳", tokens: "3.2M" },
    { index: 1, name: "general", status: "✓", tokens: "77k" },
  ]
  test("keeps full labels when they fit, then drops detail step by step", () => {
    expect(fitTabs(tabs, 200)).toEqual(tabs)
    expect(fitTabs(tabs, 46).every((t) => t.tokens === "")).toBe(true)
    expect(fitTabs(tabs, 30)[0].name).toBe("realest…")
    expect(fitTabs(tabs, 5).map((t) => t.name)).toEqual(["", ""])
  })
})
