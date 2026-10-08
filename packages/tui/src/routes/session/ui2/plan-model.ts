// U6 PlanModel: pure functions behind the architecture tab (see ARCHITECTURE-AIOVEN.md).

export type Component = { name: string; kind: "new" | "reuse"; paths: string[] }
export type FileChange = { file: string; additions: number; deletions: number; status?: "added" | "deleted" | "modified" }
export type Status = "pending" | "in_progress" | "completed"

// U7 format: - **Name** (new|reuse) — paths: a.ts, b/ — responsibility
const BULLET = /^\s*[-*]\s+\*\*(.+?)\*\*\s*\((new|reuse)\)\s*[—–-]+\s*paths?:\s*(.+?)(?:\s+[—–-]+\s+.*)?$/i

export function parsePlan(markdown: string): Component[] {
  const lines = markdown.split("\n")
  const start = lines.findIndex((line) => /^#{2,4}\s.*components/i.test(line))
  if (start === -1) return []
  const level = lines[start].match(/^#+/)![0].length
  const end = lines.findIndex((line, i) => i > start && /^#+\s/.test(line) && line.match(/^#+/)![0].length <= level)
  return lines.slice(start + 1, end === -1 ? undefined : end).flatMap((line) => {
    const match = line.match(BULLET)
    if (!match) return []
    const paths = match[3]
      .split(",")
      .map((p) => p.trim().replace(/^`|`$/g, ""))
      .filter(Boolean)
    return [{ name: match[1].trim(), kind: match[2].toLowerCase() as Component["kind"], paths }]
  })
}

const covers = (path: string, file: string) => {
  const dir = path.replace(/\/+$/, "")
  return file === dir || file.startsWith(dir + "/")
}

export function assign(components: Component[], files: FileChange[]) {
  const byComponent = new Map<string, FileChange[]>(components.map((c) => [c.name, []]))
  const other: FileChange[] = []
  for (const file of files) {
    const owner = components.find((c) => c.paths.some((p) => covers(p, file.file)))
    if (owner) byComponent.get(owner.name)!.push(file)
    else other.push(file)
  }
  return { byComponent, other }
}

export function status(component: Component, todos: ReadonlyArray<{ content: string; status: string }>): Status {
  const name = component.name.toLowerCase()
  const mine = todos.filter((t) => t.content.toLowerCase().includes(name))
  if (mine.some((t) => t.status === "in_progress")) return "in_progress"
  if (mine.length > 0 && mine.every((t) => t.status === "completed" || t.status === "cancelled")) return "completed"
  return "pending"
}

// No plan: one component per package (packages/<name>) or top-level folder.
export function infer(files: FileChange[]): Component[] {
  const groups = new Map<string, string>()
  for (const { file } of files) {
    const parts = file.split("/")
    const prefix = parts[0] === "packages" && parts.length > 2 ? `packages/${parts[1]}` : parts.length > 1 ? parts[0] : "."
    if (!groups.has(prefix)) groups.set(prefix, prefix === "." ? "project root" : prefix)
  }
  return [...groups].map(([prefix, name]) => ({ name, kind: "reuse" as const, paths: prefix === "." ? [] : [prefix] }))
}
