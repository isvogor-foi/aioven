export * as TodoNudge from "./todo-nudge"

// Agents that execute todos; recipe and subagents never get nudged.
const AGENTS = new Set(["bake"])
const MAX_WITHOUT_PROGRESS = 2

export type State = { nudges: number; done: number }

export const initial: State = { nudges: 0, done: -1 }

// Pure decision: the text to send, or undefined when the agent should stop.
export function decide(input: {
  agent: string
  isSubagent: boolean
  error: boolean
  todos: ReadonlyArray<{ content: string; status: string }>
  state: State
}): { text: string; state: State } | undefined {
  if (!AGENTS.has(input.agent) || input.isSubagent || input.error) return
  const open = input.todos.filter((t) => t.status === "pending" || t.status === "in_progress")
  if (open.length === 0) return
  const done = input.todos.filter((t) => t.status === "completed").length
  const nudges = done > input.state.done ? 0 : input.state.nudges
  if (nudges >= MAX_WITHOUT_PROGRESS) return
  return {
    text: `${open.length} todo(s) still open, next: "${open[0].content}". Continue, or mark todos cancelled if no longer needed. If blocked on the user, say so and stop.`,
    state: { nudges: nudges + 1, done },
  }
}
