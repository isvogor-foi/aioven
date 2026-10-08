import { describe, expect, test } from "bun:test"
import { TodoNudge } from "../../src/aioven/todo-nudge"

const todo = (content: string, status: string) => ({ content, status })
const base = { agent: "bake", isSubagent: false, error: false, state: TodoNudge.initial }

describe("TodoNudge.decide", () => {
  test("nudges build when todos are open, naming the next one", () => {
    const result = TodoNudge.decide({ ...base, todos: [todo("a", "completed"), todo("b", "pending")] })
    expect(result?.text).toContain('"b"')
    expect(result?.state).toEqual({ nudges: 1, done: 1 })
  })

  test("never nudges plan, subagents, errors, or finished lists", () => {
    const todos = [todo("a", "pending")]
    expect(TodoNudge.decide({ ...base, agent: "recipe", todos })).toBeUndefined()
    expect(TodoNudge.decide({ ...base, isSubagent: true, todos })).toBeUndefined()
    expect(TodoNudge.decide({ ...base, error: true, todos })).toBeUndefined()
    expect(TodoNudge.decide({ ...base, todos: [todo("a", "completed"), todo("b", "cancelled")] })).toBeUndefined()
  })

  test("stops after two nudges without progress, resets when a todo completes", () => {
    const open = [todo("a", "pending"), todo("b", "pending")]
    const first = TodoNudge.decide({ ...base, todos: open })!
    const second = TodoNudge.decide({ ...base, todos: open, state: first.state })!
    expect(TodoNudge.decide({ ...base, todos: open, state: second.state })).toBeUndefined()
    const progressed = [todo("a", "completed"), todo("b", "pending")]
    expect(TodoNudge.decide({ ...base, todos: progressed, state: second.state })?.state).toEqual({ nudges: 1, done: 1 })
  })
})
