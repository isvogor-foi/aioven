// U3 StatusBar: plan progress, ETA, tokens, cache and key hints in one line.
import { createMemo, createSignal, onCleanup, Show } from "solid-js"
import { AIOvenDefaults } from "@opencode-ai/core/aioven"
import { useSync } from "../../../context/sync"
import { useTheme } from "../../../context/theme"
import { useCommandShortcut } from "../../../keymap"
import { agents, bar, cacheHit, eta, root, seconds, tokens, usage } from "./derive"

export function StatusBar(props: { sessionID: string }) {
  const sync = useSync()
  const { theme } = useTheme()
  const send = useCommandShortcut("input.submit")
  const [now, setNow] = createSignal(Date.now())
  const timer = setInterval(() => setNow(Date.now()), 1000)
  onCleanup(() => clearInterval(timer))

  const rootID = createMemo(() => root(sync.data.session, props.sessionID))
  const plan = createMemo(() =>
    eta({
      messages: sync.data.message[rootID()] ?? [],
      parts: (id) => sync.data.part[id] ?? [],
      todos: sync.data.todo[rootID()] ?? [],
      now: now(),
    }),
  )
  const total = createMemo(() =>
    agents(sync.data, props.sessionID)
      .map((id) => usage(sync.data.message[id] ?? [], sync.session.get(id)))
      .reduce(
        (acc, u) => ({
          input: acc.input + u.input,
          output: acc.output + u.output,
          cacheRead: acc.cacheRead + u.cacheRead,
          cacheWrite: acc.cacheWrite + u.cacheWrite,
          total: acc.total + u.total,
        }),
        { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
      ),
  )
  const agent = createMemo(() => {
    const last = (sync.data.message[props.sessionID] ?? []).findLast((m) => m.role === "assistant")
    const name = (last?.role === "assistant" ? last.agent : undefined) ?? sync.session.get(props.sessionID)?.agent
    if (!name) return
    const tier = AIOvenDefaults.tier((sync.data.config as { aioven?: Parameters<typeof AIOvenDefaults.tier>[0] }).aioven, name)
    return `${name}${tier ? ` ${tier[0].toUpperCase()}` : ""}${last?.role === "assistant" ? ` ${last.modelID}` : ""}`
  })

  return (
    <box flexDirection="row" flexShrink={0} justifyContent="space-between" gap={2} paddingLeft={1}>
      <text fg={theme.textMuted} wrapMode="none" flexShrink={1}>
        <Show when={plan().total > 0}>
          <span style={{ fg: theme.text }}>PLAN </span>
          {bar(plan().done, plan().total, 5)} {plan().done}/{plan().total}
          <Show when={plan().eta !== undefined}> ~{seconds(plan().eta!)}</Show>
          {" · "}
        </Show>
        Σ {tokens(total().total)}
        <Show when={cacheHit(total()) !== undefined}> · cache {cacheHit(total())}%</Show>
        <Show when={agent()}> · {agent()}</Show>
      </text>
      <text fg={theme.textMuted} wrapMode="none" flexShrink={0}>
        <span style={{ fg: theme.text }}>{send()}</span> send · <span style={{ fg: theme.text }}>0-9</span> tabs ·{" "}
        <span style={{ fg: theme.text }}>alt+b</span> blueprint
      </text>
    </box>
  )
}
