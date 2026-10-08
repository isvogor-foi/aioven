// U4 compact(): one-line summaries of tool calls, so the transcript shows no code.
import type { ToolPart } from "@opencode-ai/sdk/v2"

export type Line = { icon: string; text: string }

const str = (value: unknown) => (typeof value === "string" ? value : undefined)
const num = (value: unknown) => (typeof value === "number" ? value : 0)

export function compact(part: ToolPart, relative: (file: string) => string = (f) => f): Line {
  const input = part.state.input ?? {}
  const meta = (part.state.status === "completed" || part.state.status === "running" ? part.state.metadata : undefined) ?? {}
  const file = relative(str(input.filePath) ?? str(input.path) ?? "")
  const failed = part.state.status === "error"
  const running = part.state.status === "running" || part.state.status === "pending"
  const pick = (done: string): string => (failed ? "✗" : running ? "⏳" : done)
  const seconds =
    part.state.status === "completed" ? ` (${Math.max(0, Math.round((part.state.time.end - part.state.time.start) / 1000))}s)` : ""

  switch (part.tool) {
    case "edit":
    case "write": {
      const diff = (meta.filediff ?? {}) as { additions?: unknown; deletions?: unknown }
      const counts = diff.additions !== undefined ? ` +${num(diff.additions)} −${num(diff.deletions)}` : ""
      return { icon: pick("✎"), text: `${part.tool === "write" ? "wrote" : "edited"} ${file}${counts}` }
    }
    case "apply_patch": {
      const files = Array.isArray(meta.files) ? (meta.files as { relativePath?: unknown; additions?: unknown; deletions?: unknown }[]) : []
      const text = files.length
        ? files.map((f) => `${str(f.relativePath) ?? "?"} +${num(f.additions)} −${num(f.deletions)}`).join(", ")
        : "patch"
      return { icon: pick("✎"), text: `edited ${text}` }
    }
    case "bash":
      return { icon: pick("⏵"), text: `${(str(input.command) ?? "").split("\n")[0]}${seconds}` }
    case "read":
      return { icon: pick("·"), text: `read ${file}` }
    case "grep":
    case "glob":
    case "ast_grep":
      return { icon: pick("·"), text: `${part.tool} ${str(input.pattern) ?? ""}` }
    case "task":
      return { icon: pick("⇢"), text: `${str(input.subagent_type) ?? "agent"}: ${str(input.description) ?? ""}` }
    case "todowrite":
      return { icon: pick("☰"), text: "updated plan" }
    default:
      return { icon: pick("·"), text: part.tool }
  }
}
