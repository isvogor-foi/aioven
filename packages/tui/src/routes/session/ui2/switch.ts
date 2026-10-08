// U2 AgentSwitch: digits on an empty prompt switch tabs; Esc/X stop agents (see ARCHITECTURE-AIOVEN.md).
import { useRenderer } from "@opentui/solid"
import { useRoute } from "../../../context/route"
import { useSDK } from "../../../context/sdk"
import { useSync } from "../../../context/sync"
import { usePromptRef } from "../../../context/prompt"
import { useBindings } from "../../../keymap"
import { useDialog } from "../../../ui/dialog"
import { DialogConfirm } from "../../../ui/dialog-confirm"
import { agents, isBusy, waitOf } from "./derive"
import { View } from "./view"

const DIGITS = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]

export function useAgentSwitch(props: { sessionID: () => string }) {
  const sync = useSync()
  const route = useRoute()
  const sdk = useSDK()
  const dialog = useDialog()
  const renderer = useRenderer()
  const promptRef = usePromptRef()

  const jump = (index: number) => {
    View.show("chat")
    const id = agents(sync.data, props.sessionID())[index]
    if (id && id !== props.sessionID()) route.navigate({ type: "session", sessionID: id })
  }

  const blocked = () =>
    dialog.stack.length > 0 ||
    !!sync.data.permission[props.sessionID()]?.length ||
    !!sync.data.question[props.sessionID()]?.length

  // Empty prompt (or no text box at all, e.g. subagent tabs).
  const idleInput = () => {
    const prompt = promptRef.current
    if (prompt?.focused) return prompt.empty ?? prompt.current.input === ""
    return renderer.currentFocusedEditor === null
  }

  const stop = async (sessionID: string) => {
    const session = sync.session.get(sessionID)
    const name = session?.parentID ? (session.agent ?? "subagent") : "main agent"
    if (await DialogConfirm.show(dialog, "Interrupt", `Stop ${name}? (y/n)`)) void sdk.client.session.abort({ sessionID })
  }

  const stopAll = async () => {
    const [rootID, ...kids] = agents(sync.data, props.sessionID())
    // Children first so the root never waits on a child that is being stopped.
    const busy = [...kids, rootID].filter((id) => isBusy(waitOf(sync.data, id)))
    if (busy.length === 0) return
    if (!(await DialogConfirm.show(dialog, "Interrupt all", `Stop ${busy.length} busy agent(s)? (y/n)`))) return
    for (const sessionID of busy) await sdk.client.session.abort({ sessionID })
  }

  useBindings(() => ({
    enabled: () => !blocked() && idleInput(),
    priority: 10,
    bindings: [
      ...DIGITS.map((digit, index) => ({ key: digit, desc: `Tab ${digit}`, group: "Agents", cmd: () => jump(index) })),
      ...["X", "shift+x"].map((key) => ({ key, desc: "Stop all agents", group: "Agents", cmd: () => void stopAll() })),
    ],
  }))

  useBindings(() => ({
    bindings: [
      ...DIGITS.map((digit, index) => ({ key: `alt+${digit}`, desc: `Tab ${digit}`, group: "Agents", cmd: () => jump(index) })),
      { key: "alt+b", desc: "Blueprint tab", group: "Agents", cmd: () => View.show("architecture") },
    ],
  }))

  // Esc without a focused text box: leave the architecture tab, stop a busy agent, or go back to tab 0.
  useBindings(() => ({
    enabled: () => !blocked() && renderer.currentFocusedEditor === null,
    priority: 10,
    bindings: [
      {
        key: "escape",
        desc: "Back / stop agent",
        group: "Agents",
        cmd: () => {
          if (View.current() === "architecture") return View.show("chat")
          if (isBusy(waitOf(sync.data, props.sessionID()))) return void stop(props.sessionID())
          jump(0)
        },
      },
    ],
  }))
}
