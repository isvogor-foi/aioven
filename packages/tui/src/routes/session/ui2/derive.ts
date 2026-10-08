import type { Message, Part, PermissionRequest, QuestionRequest, Session, SessionStatus, Todo } from "@opencode-ai/sdk/v2"

export const MAX_CHILDREN = 9

export type Wait =
  | { kind: "idle" }
  | { kind: "done" }
  | { kind: "error"; message: string }
  | { kind: "interrupted" }
  | { kind: "permission"; name: string }
  | { kind: "question" }
  | { kind: "compacting" }
  | { kind: "retry"; attempt: number; next: number }
  | { kind: "subagent"; since: number }
  | { kind: "tool"; name: string; since: number }
  | { kind: "model"; since: number }
  | { kind: "thinking" }
  | { kind: "streaming" }

export type Usage = { input: number; output: number; cacheRead: number; cacheWrite: number; total: number }

export function root(sessions: ReadonlyArray<Session>, sessionID: string) {
  const byID = new Map(sessions.map((s) => [s.id, s]))
  let current = byID.get(sessionID)
  const seen = new Set<string>()
  while (current?.parentID && byID.has(current.parentID) && !seen.has(current.id)) {
    seen.add(current.id)
    current = byID.get(current.parentID)
  }
  return current?.id ?? sessionID
}

// Direct children numbered 1..9 by creation; with more than 9, busy ones win the slots.
export function children(
  sessions: ReadonlyArray<Session>,
  rootID: string,
  busy: (sessionID: string) => boolean,
) {
  const all = sessions.filter((s) => s.parentID === rootID).toSorted((a, b) => a.time.created - b.time.created)
  if (all.length <= MAX_CHILDREN) return all
  const active = all.filter((s) => busy(s.id))
  const rest = all.filter((s) => !busy(s.id)).slice(-Math.max(0, MAX_CHILDREN - active.length))
  return [...active, ...rest].slice(0, MAX_CHILDREN).toSorted((a, b) => a.time.created - b.time.created)
}

export function usage(messages: ReadonlyArray<Message>, session?: Session): Usage {
  const sum = messages.reduce(
    (acc, msg) => {
      if (msg.role !== "assistant") return acc
      return {
        input: acc.input + msg.tokens.input,
        output: acc.output + msg.tokens.output + msg.tokens.reasoning,
        cacheRead: acc.cacheRead + msg.tokens.cache.read,
        cacheWrite: acc.cacheWrite + msg.tokens.cache.write,
      }
    },
    { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
  )
  // Session totals cover messages beyond the synced window; take whichever is larger.
  const t = session?.tokens
  const result = t
    ? {
        input: Math.max(sum.input, t.input),
        output: Math.max(sum.output, t.output + t.reasoning),
        cacheRead: Math.max(sum.cacheRead, t.cache.read),
        cacheWrite: Math.max(sum.cacheWrite, t.cache.write),
      }
    : sum
  return { ...result, total: result.input + result.output + result.cacheRead + result.cacheWrite }
}

// `input` already excludes cached tokens (see Session.getUsage).
export function cacheHit(u: Usage) {
  const prompt = u.input + u.cacheRead + u.cacheWrite
  if (prompt === 0) return undefined
  return Math.round((u.cacheRead / prompt) * 100)
}

export function steps(messages: ReadonlyArray<Message>, parts: (messageID: string) => ReadonlyArray<Part>) {
  return messages
    .filter((msg) => msg.role === "assistant")
    .reduce((acc, msg) => acc + parts(msg.id).filter((p) => p.type === "step-start").length, 0)
}

export function wait(input: {
  session?: Session
  status?: SessionStatus
  messages: ReadonlyArray<Message>
  parts: (messageID: string) => ReadonlyArray<Part>
  permissions?: ReadonlyArray<PermissionRequest>
  questions?: ReadonlyArray<QuestionRequest>
}): Wait {
  const last = input.messages.at(-1)
  const permission = input.permissions?.[0]
  if (permission) return { kind: "permission", name: permission.permission }
  if (input.questions?.length) return { kind: "question" }
  if (input.session?.time.compacting) return { kind: "compacting" }
  const status = input.status
  if (status?.type === "retry") return { kind: "retry", attempt: status.attempt, next: status.next }
  if (!status || status.type === "idle") {
    if (last?.role === "assistant" && last.error?.name === "MessageAbortedError") return { kind: "interrupted" }
    if (last?.role === "assistant" && last.error) return { kind: "error", message: errorMessage(last.error) }
    return input.session?.parentID ? { kind: "done" } : { kind: "idle" }
  }
  if (!last || last.role === "user") return { kind: "model", since: last?.time.created ?? Date.now() }
  const parts = input.parts(last.id).filter((p) => p.type !== "step-start" && p.type !== "step-finish")
  const running = parts.findLast(
    (p) => p.type === "tool" && (p.state.status === "running" || p.state.status === "pending"),
  )
  if (running?.type === "tool") {
    const since = running.state.status === "running" ? running.state.time.start : last.time.created
    if (running.tool === "task") return { kind: "subagent", since }
    return { kind: "tool", name: running.tool, since }
  }
  const tail = parts.at(-1)
  if (tail?.type === "reasoning") return { kind: "thinking" }
  if (tail?.type === "text") return { kind: "streaming" }
  const ended = parts.findLast((p) => p.type === "tool" && p.state.status !== "running" && p.state.status !== "pending")
  const since =
    ended?.type === "tool" && "time" in ended.state && "end" in ended.state.time
      ? ended.state.time.end
      : last.time.created
  return { kind: "model", since }
}

function errorMessage(error: NonNullable<Extract<Message, { role: "assistant" }>["error"]>) {
  const data = "data" in error ? (error.data as { message?: unknown }) : undefined
  return typeof data?.message === "string" ? data.message : error.name
}

export function isBusy(w: Wait) {
  return !(w.kind === "idle" || w.kind === "done" || w.kind === "error" || w.kind === "interrupted")
}

// ETA from the todowrite history: average time per completed todo × remaining todos.
export function eta(input: {
  messages: ReadonlyArray<Message>
  parts: (messageID: string) => ReadonlyArray<Part>
  todos: ReadonlyArray<Todo>
  now: number
}) {
  const snapshots = input.messages
    .flatMap((msg) => input.parts(msg.id))
    .flatMap((p) => {
      if (p.type !== "tool" || p.tool !== "todowrite" || p.state.status !== "completed") return []
      const todos = p.state.input.todos
      if (!Array.isArray(todos)) return []
      return [{ time: p.state.time.end, todos: todos as Todo[] }]
    })
    .toSorted((a, b) => a.time - b.time)
  const remaining = input.todos.filter((t) => t.status === "pending" || t.status === "in_progress").length
  const done = input.todos.filter((t) => t.status === "completed").length
  const start = snapshots[0]?.time
  const completedAt = snapshots.reduce((acc, snap) => {
    for (const todo of snap.todos) {
      if (todo.status === "completed" && !acc.has(todo.content)) acc.set(todo.content, snap.time)
    }
    return acc
  }, new Map<string, number>())
  const lastDone = Math.max(...completedAt.values())
  if (start === undefined || completedAt.size === 0 || remaining === 0 || lastDone <= start) return { done, total: done + remaining }
  const avg = (lastDone - start) / completedAt.size
  return { done, total: done + remaining, eta: Math.max(0, avg * remaining - (input.now - lastDone)) }
}

export function bar(value: number, max: number, width = 8) {
  const filled = max > 0 ? Math.min(width, Math.round((value / max) * width)) : 0
  return "▓".repeat(filled) + "░".repeat(width - filled)
}

export function tokens(n: number) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1000) return `${Math.round(n / 1000)}k`
  return String(n)
}

export function seconds(ms: number) {
  const s = Math.max(0, Math.round(ms / 1000))
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  if (m < 60) return `${m}m${s % 60 ? `${s % 60}s` : ""}`
  return `${Math.floor(m / 60)}h${m % 60}m`
}

export function label(w: Wait, now: number) {
  switch (w.kind) {
    case "idle":
      return "idle · waiting for you"
    case "done":
      return "baked"
    case "interrupted":
      return "pulled out"
    case "error":
      return `burnt: ${w.message}`
    case "permission":
      return `needs permission: ${w.name}`
    case "question":
      return "waiting for your answer"
    case "compacting":
      return "compacting context"
    case "retry":
      return `retry #${w.attempt} in ${seconds(w.next - now)}`
    case "subagent":
      return `waiting on subagent ${seconds(now - w.since)}`
    case "tool":
      return `tool:${w.name} ${seconds(now - w.since)}`
    case "model":
      return `preheating ${seconds(now - w.since)}`
    case "thinking":
      return "model thinking"
    case "streaming":
      return "model writing"
  }
}

export type Store = {
  session: ReadonlyArray<Session>
  session_status: Record<string, SessionStatus | undefined>
  message: Record<string, ReadonlyArray<Message> | undefined>
  part: Record<string, ReadonlyArray<Part> | undefined>
  permission: Record<string, ReadonlyArray<PermissionRequest> | undefined>
  question: Record<string, ReadonlyArray<QuestionRequest> | undefined>
}

export function waitOf(data: Store, sessionID: string) {
  return wait({
    session: data.session.find((s) => s.id === sessionID),
    status: data.session_status[sessionID],
    messages: data.message[sessionID] ?? [],
    parts: (messageID) => data.part[messageID] ?? [],
    permissions: data.permission[sessionID],
    questions: data.question[sessionID],
  })
}

// Panel and hotkey order: index 0 is the root session, 1..9 its subagents.
export function agents(data: Store, sessionID: string) {
  const rootID = root(data.session, sessionID)
  return [rootID, ...children(data.session, rootID, (id) => isBusy(waitOf(data, id))).map((s) => s.id)]
}
