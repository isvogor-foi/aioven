import path from "path"
import { Effect, Schema } from "effect"
import { InstanceState } from "@/effect/instance-state"
import { assertExternalDirectoryEffect } from "./external-directory"
import DESCRIPTION from "./ast-grep.txt"
import * as Tool from "./tool"

export const Parameters = Schema.Struct({
  pattern: Schema.String.annotate({ description: "ast-grep pattern, e.g. `foo($A, $$$REST)`" }),
  lang: Schema.String.annotate({ description: "Language: ts, tsx, js, py, go, rust, java, c, cpp, css, html, ..." }),
  path: Schema.optional(Schema.String).annotate({ description: "File or directory to search. Defaults to the project." }),
  rewrite: Schema.optional(Schema.String).annotate({ description: "Replacement using the pattern's metavariables" }),
  apply: Schema.optional(Schema.Boolean).annotate({ description: "Write the rewrite to files (requires rewrite)" }),
})

type Match = {
  file: string
  text: string
  lines: string
  replacement?: string
  range: { start: { line: number } }
}

const LIMIT = 100

export function format(matches: ReadonlyArray<Match>, root: string, rewrite: boolean) {
  if (matches.length === 0) return "No matches"
  const lines = matches.slice(0, LIMIT).map((m) => {
    const where = `${path.relative(root, path.resolve(root, m.file))}:${m.range.start.line + 1}`
    if (rewrite) return `${where}: ${m.text.split("\n")[0]} → ${(m.replacement ?? "").split("\n")[0]}`
    return `${where}: ${m.lines.trim()}`
  })
  if (matches.length > LIMIT) lines.push(`(${matches.length - LIMIT} more matches not shown; narrow the path or pattern)`)
  return lines.join("\n")
}

function binary() {
  const found = Bun.which("ast-grep")
  if (found) return found
  try {
    return path.join(path.dirname(require.resolve("@ast-grep/cli/package.json")), "ast-grep")
  } catch {
    return undefined
  }
}

export const AstGrepTool = Tool.define(
  "ast_grep",
  Effect.gen(function* () {
    return {
      description: DESCRIPTION,
      parameters: Parameters,
      execute: (params: Schema.Schema.Type<typeof Parameters>, ctx: Tool.Context) =>
        Effect.gen(function* () {
          const ins = yield* InstanceState.context
          const target = path.resolve(ins.directory, params.path ?? ".")
          yield* assertExternalDirectoryEffect(ctx, target, { bypass: false, kind: "directory" })
          const apply = !!params.apply && !!params.rewrite
          yield* ctx.ask({
            permission: apply ? "edit" : "grep",
            patterns: [apply ? path.relative(ins.worktree, target) || "." : params.pattern],
            always: ["*"],
            metadata: { pattern: params.pattern, rewrite: params.rewrite, path: target },
          })

          const bin = binary()
          if (!bin) {
            return {
              title: "ast-grep not installed",
              metadata: { count: 0 },
              output: "ast-grep is not installed. Install with `npm i -g @ast-grep/cli`, or use grep instead.",
            }
          }

          const args = ["run", "--pattern", params.pattern, "--lang", params.lang, "--json=compact"]
          if (params.rewrite) args.push("--rewrite", params.rewrite)
          const proc = Bun.spawn([bin, ...args, target], { cwd: ins.directory, stdout: "pipe", stderr: "pipe" })
          const [stdout, stderr, code] = yield* Effect.promise(() =>
            Promise.all([new Response(proc.stdout).text(), new Response(proc.stderr).text(), proc.exited]),
          )
          if (code !== 0 && !stdout.trim()) {
            const error = stderr
              .split("\n")
              .filter((line) => line && !line.startsWith("[warn]") && !line.startsWith("Enable postinstall"))
              .join("\n")
            throw new Error(`ast-grep failed: ${error || `exit code ${code}`}`)
          }
          const matches: Match[] = stdout.trim() ? JSON.parse(stdout) : []

          if (apply && matches.length > 0) {
            // --update-all is ignored together with --json, so drop it for the write pass.
            const write = Bun.spawn([bin, ...args.filter((a) => a !== "--json=compact"), "--update-all", target], {
              cwd: ins.directory,
              stdout: "pipe",
              stderr: "pipe",
            })
            const exit = yield* Effect.promise(() => write.exited)
            if (exit !== 0) throw new Error(`ast-grep rewrite failed with exit code ${exit}`)
          }

          return {
            title: `${params.pattern}${apply ? " (applied)" : ""}`,
            metadata: { count: matches.length },
            output: (apply ? `Rewrote ${matches.length} match(es).\n` : "") + format(matches, ins.worktree, !!params.rewrite),
          }
        }).pipe(Effect.orDie),
    }
  }),
)
