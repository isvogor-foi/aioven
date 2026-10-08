// U1 AgentTabs: one tab per agent with live status, plus the blueprint tab.
import { createEffect, createMemo, createSignal, For, onCleanup, Show } from "solid-js"
import { useRoute } from "../../../context/route"
import { useSync } from "../../../context/sync"
import { useTheme } from "../../../context/theme"
import { useTerminalDimensions } from "@opentui/solid"
import { agents, isBusy, root, seconds, tokens, usage, waitOf, type Wait } from "./derive"
import { View } from "./view"

function short(w: Wait, now: number) {
  switch (w.kind) {
    case "tool":
      return `⏳${w.name} ${seconds(now - w.since)}`
    case "subagent":
      return "⇢"
    case "model":
      return "⏳"
    case "thinking":
    case "streaming":
      return "●"
    case "permission":
      return "? permission"
    case "question":
      return "? question"
    case "retry":
      return `↻${w.attempt}`
    case "compacting":
      return "⇣"
    case "done":
      return "✓"
    case "error":
      return "✗"
    case "interrupted":
      return "■"
    case "idle":
      return ""
  }
}

export type TabLabel = { index: number; name: string; status: string; tokens: string }

// Pick the most detailed level at which all tabs fit: full → no tokens → short names → index + status.
export function fitTabs(tabs: TabLabel[], width: number) {
  const levels = [
    (t: TabLabel) => ({ ...t }),
    (t: TabLabel) => ({ ...t, tokens: "" }),
    (t: TabLabel) => ({ ...t, tokens: "", name: t.name.length > 8 ? t.name.slice(0, 7) + "…" : t.name }),
    (t: TabLabel) => ({ ...t, tokens: "", name: "" }),
  ]
  const size = (t: TabLabel) =>
    [String(t.index), t.name, t.status, t.tokens].filter(Boolean).join(" ").length + 3
  for (const level of levels) {
    const fitted = tabs.map(level)
    if (fitted.reduce((sum, t) => sum + size(t), 0) <= width) return fitted
  }
  return tabs.map(levels[3])
}

export function AgentTabs(props: { sessionID: string; architecture: { done: number; total: number } }) {
  const sync = useSync()
  const route = useRoute()
  const { theme } = useTheme()
  const [now, setNow] = createSignal(Date.now())
  const timer = setInterval(() => setNow(Date.now()), 1000)
  onCleanup(() => clearInterval(timer))

  // Subagent sessions may be missing from the session list; task tool parts point at them.
  const rootID = createMemo(() => root(sync.data.session, props.sessionID))
  const taskSessions = createMemo(() =>
    (sync.data.message[rootID()] ?? [])
      .flatMap((msg) => sync.data.part[msg.id] ?? [])
      .flatMap((p) => {
        if (p.type !== "tool" || p.tool !== "task" || !("metadata" in p.state)) return []
        const id = p.state.metadata?.sessionId
        return typeof id === "string" ? [id] : []
      }),
  )
  createEffect(() => {
    // The session route loads the root itself; only fetch subagents here.
    for (const id of taskSessions()) {
      if (!sync.session.get(id) || !sync.data.message[id]?.length) void sync.session.sync(id)
    }
  })

  const tabs = createMemo(() =>
    agents(sync.data, props.sessionID).map((id, index) => {
      const session = sync.session.get(id)
      const messages = sync.data.message[id] ?? []
      const last = messages.findLast((m) => m.role === "assistant")
      const w = waitOf(sync.data, id)
      const name = (last?.role === "assistant" ? last.agent : undefined) ?? session?.agent ?? (index === 0 ? "main" : "agent")
      return { id, index, name, wait: w, tokens: usage(messages, session).total }
    }),
  )

  const dimensions = useTerminalDimensions()
  const archLabel = () =>
    `B blueprint${props.architecture.total > 0 ? ` ${props.architecture.done}/${props.architecture.total}` : ""}`
  const labels = createMemo(() =>
    fitTabs(
      tabs().map((t) => ({
        index: t.index,
        name: t.name,
        status: short(t.wait, now()),
        tokens: t.tokens > 0 ? tokens(t.tokens) : "",
      })),
      dimensions().width - archLabel().length - 8,
    ),
  )

  const color = (w: Wait) => {
    if (w.kind === "permission" || w.kind === "question") return theme.warning
    if (w.kind === "error" || w.kind === "retry") return theme.error
    if (w.kind === "done") return theme.success
    if (isBusy(w)) return theme.accent
    return theme.textMuted
  }

  return (
    <box flexDirection="row" flexShrink={0} justifyContent="space-between" gap={1}>
      <box flexDirection="row" gap={0} flexShrink={1} overflow="hidden">
        <For each={tabs()}>
          {(tab) => {
            const active = () => View.current() === "chat" && tab.id === props.sessionID
            return (
              <box
                flexShrink={0}
                paddingLeft={1}
                paddingRight={1}
                backgroundColor={active() ? theme.backgroundElement : undefined}
                onMouseUp={() => {
                  View.show("chat")
                  if (tab.id !== props.sessionID) route.navigate({ type: "session", sessionID: tab.id })
                }}
              >
                <text fg={active() ? theme.text : theme.textMuted} wrapMode="none">
                  <span style={{ fg: theme.textMuted }}>{tab.index}</span>
                  <Show when={labels()[tab.index]?.name}>
                    {" "}
                    {active() ? <b>{labels()[tab.index].name}</b> : labels()[tab.index].name}
                  </Show>
                  <Show when={labels()[tab.index]?.status}>
                    <span style={{ fg: color(tab.wait) }}> {labels()[tab.index].status}</span>
                  </Show>
                  <Show when={labels()[tab.index]?.tokens}>
                    <span style={{ fg: theme.textMuted }}> {labels()[tab.index].tokens}</span>
                  </Show>
                </text>
              </box>
            )
          }}
        </For>
      </box>
      <box
        flexShrink={0}
        paddingLeft={1}
        paddingRight={1}
        backgroundColor={View.current() === "architecture" ? theme.backgroundElement : undefined}
        onMouseUp={() => View.show("architecture")}
      >
        <text fg={View.current() === "architecture" ? theme.text : theme.textMuted} wrapMode="none">
          {archLabel()}
        </text>
      </box>
    </box>
  )
}
