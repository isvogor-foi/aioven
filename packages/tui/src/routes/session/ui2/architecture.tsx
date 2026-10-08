// U5 ArchitectureView: components being built and the files they change (full-screen tab).
import path from "path"
import { createMemo, createResource, For, Show, type Accessor } from "solid-js"
import { useProject } from "../../../context/project"
import { useSDK } from "../../../context/sdk"
import { useSync } from "../../../context/sync"
import { useTheme } from "../../../context/theme"
import { root } from "./derive"
import { assign, infer, parsePlan, status, type Component, type FileChange, type Status } from "./plan-model"
import { View } from "./view"

export type ArchitectureModel = {
  source: { kind: "plan"; file: string } | { kind: "inferred" }
  components: { component: Component; status: Status; files: FileChange[] }[]
  other: FileChange[]
  done: number
  total: number
}

export function useArchitecture(sessionID: () => string): Accessor<ArchitectureModel> {
  const sync = useSync()
  const sdk = useSDK()
  const project = useProject()
  const rootID = createMemo(() => root(sync.data.session, sessionID()))
  const files = createMemo<FileChange[]>(() =>
    (sync.data.session_diff[rootID()] ?? []).flatMap((d) =>
      d.file ? [{ file: d.file, additions: d.additions, deletions: d.deletions, status: d.status }] : [],
    ),
  )
  const todos = createMemo(() => sync.data.todo[rootID()] ?? [])
  const planFile = createMemo(() => {
    const session = sync.session.get(rootID())
    if (!session) return
    return path.join(".opencode", "plans", `${session.time.created}-${session.slug}.md`)
  })
  // Re-read the plan when progress changes or the tab opens.
  const trigger = createMemo(() =>
    [planFile(), todos().map((t) => t.status).join(), files().length, View.current()].join("|"),
  )
  const [markdown] = createResource(trigger, async () => {
    const file = planFile()
    if (!file) return
    const { worktree, directory } = project.instance.path()
    const target = path.relative(directory || ".", path.join(worktree || directory, file))
    const res = await sdk.client.file.read({ path: target }).catch(() => undefined)
    return res?.data?.type === "text" ? res.data.content : undefined
  })

  return createMemo(() => {
    const planned = markdown.latest ? parsePlan(markdown.latest) : []
    const components = planned.length > 0 ? planned : infer(files())
    const { byComponent, other } = assign(components, files())
    const rows = components.map((component) => ({
      component,
      status: planned.length > 0 ? status(component, todos()) : ("in_progress" as Status),
      files: byComponent.get(component.name) ?? [],
    }))
    return {
      source: planned.length > 0 ? { kind: "plan" as const, file: path.basename(planFile() ?? "") } : { kind: "inferred" as const },
      components: planned.length > 0 ? rows : rows.filter((r) => r.files.length > 0),
      other,
      done: planned.length > 0 ? rows.filter((r) => r.status === "completed").length : 0,
      total: planned.length,
    }
  })
}

const ICON: Record<Status, string> = { completed: "✓", in_progress: "⏳", pending: "○" }

export function ArchitectureView(props: { model: ArchitectureModel }) {
  const { theme } = useTheme()
  const statusColor = (s: Status) => (s === "completed" ? theme.success : s === "in_progress" ? theme.accent : theme.textMuted)
  const counts = (f: FileChange) => (
    <>
      <span style={{ fg: theme.diffAdded }}> +{f.additions}</span>
      <span style={{ fg: theme.diffRemoved }}> −{f.deletions}</span>
    </>
  )

  return (
    <scrollbox flexGrow={1}>
      <box gap={1} paddingTop={1}>
        <text fg={theme.text} wrapMode="none">
          <b>BLUEPRINT</b>
          <span style={{ fg: theme.textMuted }}>
            {"  "}
            {props.model.source.kind === "plan" ? `from recipe ${props.model.source.file}` : "from changed files"}
          </span>
          <Show when={props.model.total > 0}>
            <span style={{ fg: theme.textMuted }}>
              {"  "}
              {props.model.done}/{props.model.total} components
            </span>
          </Show>
        </text>
        <Show when={props.model.components.length === 0 && props.model.other.length === 0}>
          <text fg={theme.textMuted}>No plan and no changed files yet.</text>
        </Show>
        <For each={props.model.components}>
          {(row) => (
            <box>
              <text wrapMode="none">
                <span style={{ fg: statusColor(row.status) }}>{ICON[row.status]} </span>
                <b>{row.component.name}</b>
                <span style={{ fg: theme.textMuted }}> {row.component.kind}</span>
              </text>
              <Show
                when={row.files.length > 0}
                fallback={
                  <text fg={theme.textMuted} wrapMode="none">
                    {"    "}
                    {row.component.paths.join(", ") || "no files yet"}
                  </text>
                }
              >
                <For each={row.files}>
                  {(f) => (
                    <text fg={theme.text} wrapMode="none">
                      {"    "}
                      {f.file}
                      {counts(f)}
                    </text>
                  )}
                </For>
              </Show>
            </box>
          )}
        </For>
        <Show when={props.model.other.length > 0}>
          <box>
            <text fg={theme.textMuted}>
              <b>OTHER CHANGED FILES</b>
            </text>
            <For each={props.model.other}>
              {(f) => (
                <text fg={theme.text} wrapMode="none">
                  {"    "}
                  {f.file}
                  {counts(f)}
                </text>
              )}
            </For>
          </box>
        </Show>
      </box>
    </scrollbox>
  )
}
