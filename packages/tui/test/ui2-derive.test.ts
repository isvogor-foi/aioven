import { describe, expect, test } from "bun:test"
import type { Message, Part, Session, Todo } from "@opencode-ai/sdk/v2"
import {
  bar,
  cacheHit,
  children,
  eta,
  label,
  root,
  steps,
  usage,
  wait,
} from "../src/routes/session/ui2/derive"

const session = (id: string, created: number, extra: Partial<Session> = {}) =>
  ({ id, title: id, time: { created, updated: created }, ...extra }) as Session

const tok = (input: number, output: number, read = 0, write = 0) => ({
  input,
  output,
  reasoning: 0,
  cache: { read, write },
})

const user = (id: string, created: number) => ({ id, role: "user", time: { created } }) as unknown as Message
const assistant = (id: string, created: number, extra: Record<string, unknown> = {}) =>
  ({ id, role: "assistant", time: { created }, tokens: tok(0, 0), ...extra }) as unknown as Message

const tool = (name: string, state: Record<string, unknown>) => ({ type: "tool", tool: name, state }) as unknown as Part
const partsOf = (map: Record<string, Part[]>) => (id: string) => map[id] ?? []

describe("root/children", () => {
  const sessions = [
    session("root", 1),
    session("c1", 2, { parentID: "root" }),
    session("c2", 3, { parentID: "root" }),
    session("g1", 4, { parentID: "c1" }),
  ]

  test("walks up to the root from a nested child", () => {
    expect(root(sessions, "g1")).toBe("root")
    expect(root(sessions, "root")).toBe("root")
    expect(root(sessions, "unknown")).toBe("unknown")
  })

  test("lists direct children by creation time", () => {
    expect(children(sessions, "root", () => false).map((s) => s.id)).toEqual(["c1", "c2"])
  })

  test("keeps busy children when there are more than nine", () => {
    const many = [session("r", 0), ...Array.from({ length: 12 }, (_, i) => session(`k${i}`, i + 1, { parentID: "r" }))]
    const result = children(many, "r", (id) => id === "k0").map((s) => s.id)
    expect(result).toHaveLength(9)
    expect(result[0]).toBe("k0")
    expect(result.at(-1)).toBe("k11")
  })
})

describe("usage", () => {
  test("sums assistant messages and computes cache hit", () => {
    const u = usage([
      user("u", 1),
      assistant("a1", 2, { tokens: tok(100, 10, 300) }),
      assistant("a2", 3, { tokens: tok(50, 5, 450, 100) }),
    ])
    expect(u).toEqual({ input: 150, output: 15, cacheRead: 750, cacheWrite: 100, total: 1015 })
    expect(cacheHit(u)).toBe(75)
  })

  test("prefers session totals when larger than the synced window", () => {
    const u = usage([assistant("a", 1, { tokens: tok(10, 1) })], session("s", 1, { tokens: tok(1000, 100, 0, 0) }))
    expect(u.total).toBe(1100)
  })

  test("no prompt tokens means no cache figure", () => {
    expect(cacheHit({ input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 })).toBeUndefined()
  })
})

describe("wait", () => {
  const busy = { type: "busy" } as const

  test("permission and question beat everything", () => {
    const base = { messages: [], parts: partsOf({}), status: busy }
    expect(wait({ ...base, permissions: [{ permission: "bash" } as never] })).toEqual({
      kind: "permission",
      name: "bash",
    })
    expect(wait({ ...base, questions: [{} as never] }).kind).toBe("question")
  })

  test("idle root waits for user, idle child is done, errors surface", () => {
    expect(wait({ messages: [], parts: partsOf({}), session: session("r", 1) }).kind).toBe("idle")
    expect(wait({ messages: [], parts: partsOf({}), session: session("c", 1, { parentID: "r" }) }).kind).toBe("done")
    const failed = assistant("a", 1, { error: { name: "APIError", data: { message: "rate limited" } } })
    expect(wait({ messages: [failed], parts: partsOf({}) })).toEqual({ kind: "error", message: "rate limited" })
    const aborted = assistant("b", 1, { error: { name: "MessageAbortedError", data: { message: "Aborted" } } })
    expect(wait({ messages: [aborted], parts: partsOf({}) })).toEqual({ kind: "interrupted" })
  })

  test("retry carries attempt and next", () => {
    expect(wait({ messages: [], parts: partsOf({}), status: { type: "retry", attempt: 2, message: "x", next: 99 } })).toEqual(
      { kind: "retry", attempt: 2, next: 99 },
    )
  })

  test("running tool, subagent, streaming and waiting for model", () => {
    const msgs = [user("u", 1), assistant("a", 5)]
    const run = (parts: Part[]) => wait({ messages: msgs, parts: partsOf({ a: parts }), status: busy })
    expect(run([tool("bash", { status: "running", time: { start: 7 } })])).toEqual({ kind: "tool", name: "bash", since: 7 })
    expect(run([tool("task", { status: "running", time: { start: 8 } })])).toEqual({ kind: "subagent", since: 8 })
    expect(run([{ type: "text" } as Part]).kind).toBe("streaming")
    expect(run([{ type: "reasoning" } as Part]).kind).toBe("thinking")
    expect(run([{ type: "step-start" } as Part])).toEqual({ kind: "model", since: 5 })
    expect(run([tool("read", { status: "completed", time: { start: 6, end: 9 } })])).toEqual({ kind: "model", since: 9 })
    expect(wait({ messages: [user("u", 3)], parts: partsOf({}), status: busy })).toEqual({ kind: "model", since: 3 })
  })
})

describe("steps", () => {
  test("counts step-start parts of assistant messages", () => {
    const parts = partsOf({ a: [{ type: "step-start" } as Part, { type: "step-start" } as Part], b: [{ type: "step-start" } as Part] })
    expect(steps([user("u", 1), assistant("a", 2), assistant("b", 3)], parts)).toBe(3)
  })
})

describe("eta", () => {
  const todo = (content: string, status: string) => ({ content, status, priority: "medium" }) as Todo
  const snap = (end: number, todos: Todo[]) => tool("todowrite", { status: "completed", input: { todos }, time: { start: end, end } })

  test("hidden until the first todo completes", () => {
    const parts = partsOf({ a: [snap(0, [todo("x", "pending"), todo("y", "pending")])] })
    const result = eta({ messages: [assistant("a", 0)], parts, todos: [todo("x", "pending"), todo("y", "pending")], now: 10 })
    expect(result).toEqual({ done: 0, total: 2 })
  })

  test("average time per completed todo times remaining", () => {
    const parts = partsOf({
      a: [
        snap(0, [todo("x", "in_progress"), todo("y", "pending"), todo("z", "pending")]),
        snap(60_000, [todo("x", "completed"), todo("y", "in_progress"), todo("z", "pending")]),
      ],
    })
    const todos = [todo("x", "completed"), todo("y", "in_progress"), todo("z", "pending")]
    expect(eta({ messages: [assistant("a", 0)], parts, todos, now: 60_000 })).toEqual({ done: 1, total: 3, eta: 120_000 })
    expect(eta({ messages: [assistant("a", 0)], parts, todos, now: 90_000 }).eta).toBe(90_000)
  })
})

describe("formatting", () => {
  test("bar clamps and fills", () => {
    expect(bar(0, 100)).toBe("░░░░░░░░")
    expect(bar(50, 100)).toBe("▓▓▓▓░░░░")
    expect(bar(500, 100)).toBe("▓▓▓▓▓▓▓▓")
  })

  test("labels", () => {
    expect(label({ kind: "tool", name: "bash", since: 0 }, 12_000)).toBe("tool:bash 12s")
    expect(label({ kind: "retry", attempt: 1, next: 5_000 }, 0)).toBe("retry #1 in 5s")
  })
})
