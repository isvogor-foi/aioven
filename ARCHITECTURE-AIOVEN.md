# AIOven architecture

AIOven is built from components: each component has one responsibility and a written interface. This document defines each component's interface and how the components communicate. A component's interface is written here before its code.

The rules:
1. **Interfaces first.** A new component gets an entry here (responsibility, interface, communication) before any code is written.
2. **Narrow, one-directional dependencies.** TUI → SDK/core. Server → core. Core depends on neither. No component reads another component's internals.
3. **AIOven code lives in its own files**: `packages/opencode/src/aioven/`, `packages/core/src/aioven.ts`, `packages/tui/src/routes/session/agents-panel/`, `packages/tui/src/context/nav.ts`. Edits to upstream files are only small hook calls, so merges from upstream stay cheap.
4. **Pure logic is separated from I/O and UI.** Pure functions are unit-tested; components only wire them up.

## Component map

```
            ┌──────────────────────── packages/core ────────────────────────┐
            │  AIOvenDefaults (aioven.ts)      ConfigV1.aioven (schema)     │
            └───────────▲──────────────────────────────▲────────────────────┘
                        │ import                       │ import
  ┌──────── packages/opencode (server) ───────┐  ┌──────── packages/tui (client) ────────┐
  │ AIOven (aioven/index.ts)                  │  │ Nav (context/nav.ts)                  │
  │   ├─▶ Agent registry (agent/agent.ts)     │  │ AgentsPanel (agents-panel/index.tsx)  │
  │   └─▶ Session loop (session/prompt.ts)    │  │   └─▶ Derive (agents-panel/derive.ts) │
  │ Subagents: explore/review/test-runner/    │  │ AgentKeys (agents-panel/keys.tsx)     │
  │   research (agent/prompt/*.txt)           │  │   ├─▶ Derive                          │
  │ Plan mode (session/prompt/plan-mode.txt)  │  │   └─▶ Nav                             │
  │ [P5] CodeTools  [P5] TodoNudge            │  │ Prompt (component/prompt) ─▶ Nav      │
  │ [P5] DirInstructions  [P6] CacheGuard     │  │ Permission prompt (hotkeys)           │
  └──────────────────┬────────────────────────┘  └──────────────────▲────────────────────┘
                     │  HTTP API + event stream (SDK types)          │
                     └───────────────────────────────────────────────┘
```

## Built components

### AIOvenDefaults — `packages/core/src/aioven.ts`
The single source of the defaults that both server and TUI use.
```ts
type Tier = "small" | "medium" | "large"
const AGENTS: Record<string, { tier: Tier; budget: number }>
const VARIANT: Record<Tier, string>                 // reasoning effort per tier
function budget(settings, agentName, isSubagent): number
```
Communication: imported by AIOven (server) and AgentsPanel (TUI). It is pure and has no I/O.

### Config schema — `ConfigV1.Info.aioven` (`packages/core/src/v1/config/config.ts`)
```ts
aioven?: { tiers?: { small?: string; medium?: string; large?: string }
           terse?: "off" | "lite" | "full" | "ultra"
           agents?: Record<string, { tier?: Tier; budget?: number }> }
```
Communication: the server reads it through `Config.Service`. The TUI reads it from `sync.data.config` (the same object, sent over the SDK).

### AIOven — `packages/opencode/src/aioven/index.ts`
Resolves the settings for each agent and supplies the terse-style text.
```ts
function agent(settings, name): { tier; budget?; model?: string; variant } | undefined
function terse(settings): string | undefined        // deterministic → cache-safe
```
Communication: called synchronously by the agent registry (when agents are built) and by the session loop (when the system prompt is assembled).

### Agent registry hook — `agent/agent.ts`
- Built-in agents: `build` and `plan` (main agents); `explore`, `review`, `test-runner` and `research` (read-only subagents, enforced by permission rules).
- Hook: before user config overrides are applied, `AIOven.agent()` sets `model` (from the tier) and the default `variant`. An explicit `agent.<name>.model` in config wins.

### Session loop hook — `session/prompt.ts`
- Hook: the `AIOven.terse()` text is placed first in the per-request system parts, so it stays inside the cacheable prefix.

### Plan mode — `session/prompt/plan-mode.txt`, `build-switch.txt`
- Interface: the plan file must contain the sections Context, Architecture (Components, Interfaces, Communication), Implementation steps and Verification.
- Communication: `plan_exit` asks the user for approval and switches to `build`. The build-switch reminder tells `build` to turn the steps into todos and to implement the Architecture interfaces exactly.

### Derive — `packages/tui/src/routes/session/agents-panel/derive.ts`
Pure functions over SDK data.
```ts
type Store = { session; session_status; message; part; permission; question }   // subset of the sync store
type Wait = { kind: "idle" | "done" | "error" | "interrupted" | "permission" | "question"
                   | "compacting" | "retry" | "subagent" | "tool" | "model" | "thinking" | "streaming"; … }
function agents(data: Store, sessionID): string[]          // [root, child1..child9] = hotkey order
function waitOf(data: Store, sessionID): Wait
function usage(messages, session?): Usage                 // tokens in/out/cache
function eta({ messages, parts, todos, now }): { done; total; eta? }
function cacheHit(u), steps(…), label(w, now), bar(…), tokens(n), seconds(ms)
```
Communication: none of its own. AgentsPanel and AgentKeys call it.

### Nav — `packages/tui/src/context/nav.ts`
The navigation-mode state.
```ts
Nav.active(): boolean   Nav.enter(): void   Nav.exit(): void
```
Communication: a Solid signal. Prompt reads it (to blur and to show the NAV label) and calls `enter()` on Esc. AgentKeys reads it (to enable its keys) and calls `exit()`. AgentsPanel reads it (to show the hint).

### AgentsPanel — `agents-panel/index.tsx`
- Interface: `<AgentsPanel sessionID />`, rendered by `sidebar.tsx`.
- Communication: reads the sync store (which the server's event stream updates). Calls `sync.session.sync(id)` for child sessions that aren't loaded yet. Calls `route.navigate` on click.

### AgentKeys — `agents-panel/keys.tsx`
- Interface: `useAgentKeys({ sessionID: () => string })`, called once in the `Session` route.
- Communication:
  - registers key layers in the keymap (Alt+0–9 always; NAV keys while `Nav.active()` and no editor is focused);
  - calls `route.navigate`;
  - opens a `DialogConfirm`, then `sdk.client.session.abort`;
  - dispatches the commands `model.list`, `session.line.up` and `session.line.down`.

### Permission hotkeys — `routes/session/permission.tsx`
- Interface: `HOTKEYS: Record<optionKey, key>` (once→y, always→a, reject→n, confirm→y, cancel→n).
- Communication: each hotkey calls the prompt's existing `onSelect(option)`. Nothing new is sent to the server.

## Phase 5–6 components (built to the interfaces below)

### [P5] TodoNudge — `packages/opencode/src/aioven/todo-nudge.ts` ✓
Responsibility: when the main agent stops while it still has open todos, send it one short "continue" prompt.
```ts
type State = { nudges: number; done: number }
function decide(input: { agent: string; isSubagent: boolean; error: boolean;
                         todos: { content: string; status: string }[]; state: State })
  : { text: string; state: State } | undefined
const MAX_WITHOUT_PROGRESS = 2
```
Communication (changed from the first draft: there is no bus subscriber, because nothing in the codebase subscribes to idle events; the loop exit is the natural hook):
- At the session loop's exit point (`session/prompt.ts`), the loop reads the todos from `Todo.Service` and calls `decide()`.
- If `decide()` returns a nudge, the loop writes a synthetic user message and continues; otherwise it exits as before.
- It never fires for `plan`, subagents, or after an error, and stops after 2 nudges without a todo being completed.

### [P5] DirInstructions — `packages/opencode/src/aioven/dir-instructions.ts`
Responsibility: the first time a file in a directory is read, attach that directory's `AGENTS.md` once per session.
```ts
interface DirInstructions {
  // given a file just read and the set of directories already injected → new files to attach
  pending(input: { file: string; root: string; seen: ReadonlySet<string>;
                   exists: (p: string) => boolean }): string[]
}
```
- Communication: called from the read tool's `tool.execute.after` path. It returns paths; the caller appends their contents to the tool output. Root `AGENTS.md` and README files are excluded, because the session already loads the root instructions.
- **Resolved: reuse.** Upstream `session/instruction.ts` (`Instruction.resolve`, called from `tool/read.ts`) already attaches nested `AGENTS.md` once per directory per message chain. No new component is needed.

### [P5] CodeTools — LSP and ast-grep
- **LSP:** reuse the existing `lsp` tool. Turn it on by default (`experimentalLspTool`) and allow it for `build`, `plan`, `explore` and `review`.
- **ast-grep:** a new tool `ast_grep`.
  ```ts
  parameters: { pattern: string; lang: string; path?: string; rewrite?: string; apply?: boolean }
  result: "path:line: match" lines (capped) | diff preview when rewrite is given
  ```
  - It runs the `ast-grep` binary: the one on PATH, else the `@ast-grep/cli` dependency. If neither exists, it returns a clear "not installed" message.
  - Searching asks the `grep` permission; `apply: true` asks `edit`. `explore` and `review` allow `ast_grep` but deny `edit`, so they can only search.
  - `format(matches, root, rewrite)` is pure and unit-tested. ✓

### [P6] CacheGuard ✓
- Test-only component: scans `aioven/`, `agent/prompt/` and the system-prompt assembly for non-deterministic values (`Date.now`, `Math.random`, `randomUUID`) inside prompt text.
- Interface: `test/aioven/cache-safety.test.ts`.
- Communication: none at runtime.

### [P6] Small-tier internals ✓
- Title, summary and compaction requests use the `small` tier model when `aioven.tiers.small` is set.
- Communication: `Provider.getSmallModel` falls back to `aioven.tiers.small` when `small_model` is unset (used for titles). The `title` and `summary` agents get tier `small` in `AIOvenDefaults.AGENTS`.
- Compaction deliberately stays on the session model: summarising a long context with a small model loses too much.

## Phase 7 and leftovers (interfaces defined before code)

### [L1] Interrupt all — `agents-panel/keys.tsx`
Responsibility: stop every busy agent in the current session tree.
```ts
// NAV mode, Shift+X: confirm "Stop N busy agents? (y/n)" then abort each busy session
function interruptAll(): Promise<void>
```
- Communication: uses `Derive.agents()` and `Derive.waitOf()` to find the busy session IDs, `DialogConfirm` to confirm, then calls `sdk.client.session.abort` for each.
- Children are aborted before the root, so the parent never waits on a child that is being stopped.

### [L2] Tier badge on cards — `agents-panel/index.tsx` + `AIOvenDefaults`
```ts
// core/src/aioven.ts
function tier(settings, agentName): Tier | undefined     // config override, then default
```
- Card header: `0 ● build M sonnet`. S/M/L is the agent's tier.
- Communication: AgentsPanel reads `sync.data.config.aioven` and calls `AIOvenDefaults.tier()`. No server change.

### [L3] Send hint — `component/prompt/index.tsx`
- When the prompt has text, the status line shows `⇧⏎ send` (or `alt+⏎ send`), taken from the `input.submit` binding via the existing `useCommandShortcut`.
- Communication: read-only use of the keymap. It shows the user's own binding, not a hard-coded key.

### [P7] Big-task mode — not built
The todo nudge, plan mode → todos, and background subagents together already cover the "keep working through the plan" loop. A separate plan-file loop would add a second source of truth for progress. Revisit only if the todo nudge proves insufficient in real use.

### [—] `/terse` slash command — not built
Terse level is server config (`aioven.terse`). Changing it at runtime needs a config write path. Until there is one, set it in `opencode.json`.

---

# UI v2 — design (agreed before coding)

Requirements (user, 2026-10-08):
- **Tabs per agent** at the top; each tab title carries live status.
- **Switching agents:** digits on an **empty** prompt switch tabs (`0` = main); typed text keeps digits as text. There is no NAV mode.
- **Minimal code on screen.** Show the list of changing files and the components being built, ideally as an architecture view.
- **Fix the clutter, the hidden agent status, the look & feel, and the confusing navigation.**

## Screen layout (revised 2026-10-08: architecture is its own tab, chat is full width)

Chat tab (tab 0, or any agent tab):
```
 [0 build ⏳bash 12s] [1 explore ● 7k] [2 review ✓] [3 test ✗]      [A architecture 3/5]
──────────────────────────────────────────────────────────────────────────────────────
 you: add retry to the fetch client
 build: plan approved, 5 steps
   ✓ read client.ts
   ✎ edited src/retry.ts +40 −0
   ⏳ bun test (12s)
──────────────────────────────────────────────────────────────────────────────────────
 > _
 PLAN ▓▓▓░░ 3/5 ~4m · Σ 62k · cache 84% · build M sonnet         ⇧⏎ send · 0-9 tabs · alt+a arch
```

Architecture tab (Alt+A or click; Esc / 0 returns to chat):
```
 [0 build ⏳] [1 explore ●] [2 review ✓]                            [A architecture 3/5]
──────────────────────────────────────────────────────────────────────────────────────
 ARCHITECTURE  from plan: 2026-10-08-retry.md                        3/5 components
 ✓ RetryPolicy   new     src/retry.ts            +40 −0
 ⏳ FetchClient   reuse   src/client.ts           +8 −2
 ○ ClientTests   new     test/client.test.ts
 OTHER CHANGED FILES
                         bun.lock                +3 −0
──────────────────────────────────────────────────────────────────────────────────────
 > _
```
With no plan, components are inferred from the folders of the changed files (`from changed files`), plus the same file list.

## Components

| # | Component | New / reuse | Location |
|---|---|---|---|
| U1 | AgentTabs | new (replaces the AgentsPanel cards) | `tui/src/routes/session/ui2/tabs.tsx` |
| U2 | AgentSwitch | new (replaces Nav + NAV keys) | `tui/src/routes/session/ui2/switch.ts` |
| U3 | StatusBar | new | `tui/src/routes/session/ui2/status-bar.tsx` |
| U4 | CompactTranscript | reuse the message renderer + a new compact mode | `routes/session/index.tsx` (small hook) + `ui2/compact.ts` |
| U5 | ArchitectureView | new: full-screen tab "A" (the right sidebar is removed) | `tui/src/routes/session/ui2/architecture.tsx` |
| U8 | ViewState | new: which view is shown, `chat` or `architecture` | `tui/src/routes/session/ui2/view.ts` |
| U6 | PlanModel | new, pure | `tui/src/routes/session/ui2/plan-model.ts` |
| U7 | Plan format contract | change to the plan-mode prompt | `opencode/src/session/prompt/plan-mode.txt` |
| — | Derive | reuse unchanged | `agents-panel/derive.ts` → moved to `ui2/derive.ts` |

## Interfaces

### U6 PlanModel (pure, unit-tested)
```ts
type Component = { name: string; kind: "new" | "reuse"; paths: string[] }
type FileChange = { file: string; additions: number; deletions: number; status?: "added" | "deleted" | "modified" }
type Status = "pending" | "in_progress" | "completed"

function parsePlan(markdown: string): Component[]
// Reads the "### Components" list of the plan's Architecture section, in the format fixed by U7.

function assign(components: Component[], files: FileChange[]): { byComponent: Map<string, FileChange[]>; other: FileChange[] }
// A file belongs to the first component whose path equals it or is a directory prefix of it.

function status(component: Component, todos: { content: string; status: string }[]): Status
// A todo that mentions the component name sets the status; otherwise "pending".

function infer(files: FileChange[]): Component[]
// No plan: one "reuse" component per package (packages/<name>) or top-level folder of the changed files.
```

### U8 ViewState
```ts
const View: { current(): "chat" | "architecture"; show(v: "chat" | "architecture"): void }
```
- Communication: a Solid signal.
  - The session route renders either the transcript or ArchitectureView depending on it.
  - AgentTabs highlights the matching tab.
  - AgentSwitch sets it: Alt+A → architecture; a digit, Esc or `0` → chat.

### U7 Plan format contract (plan-mode prompt)
Each component is one bullet, exactly:
`- **<Name>** (new|reuse) — paths: <path>[, <path>…] — <responsibility>`
Every implementation step names its component, so the todos name their components too.

### U5 ArchitectureView (full-screen tab)
```ts
type ArchitectureModel = {
  source: { kind: "plan"; file: string } | { kind: "inferred" }
  components: { component: Component; status: Status; files: FileChange[] }[]
  other: FileChange[]
  done: number; total: number
}
function useArchitecture(sessionID: () => string): Accessor<ArchitectureModel>   // shared by the view and the tab label
```
- Plan file: `.opencode/plans/<session.time.created>-<session.slug>.md` of the root session, read with `sdk.client.file.read`. It is re-read when the todos or diffs change and when the tab opens.
- If the file is missing or unparsable, or has no components, the model falls back to `infer(files)`.

`<ArchitectureView sessionID />` renders in place of the transcript when `View.current() === "architecture"`.
- Reads: the plan markdown via `sdk.client.file.read({ path })` (the plan file of the root session), `sync.data.session_diff[rootID]` and `sync.data.todo[rootID]`.
- Renders: the components with status icon, kind and their files (+/−), then "other files".
- No plan yet: it shows `PlanModel.infer(files)` as the components, labelled "from changed files", plus the file list.

### U1 AgentTabs
`<AgentTabs sessionID />`
- Tab i = `Derive.agents(store, sessionID)[i]`.
- Title: `<i> <agent> <short wait label> <tokens>`. The active tab is highlighted; a tab whose agent needs permission or a question gets the warning colour.
- Click → `route.navigate` (and `View.show("chat")`).
- `fitTabs(labels, width)` (pure) picks the most detailed level that fits: full → no tokens → names shortened to 8 characters → index + status only.
- Also responsible for loading subagent sessions. Old sessions' children may be missing from the session list, so it collects their IDs from the root's `task` tool parts and calls `sync.session.sync(id)`. Found in the live check.
- A fixed `[A architecture <done>/<total>]` tab on the right; click → `View.show("architecture")`.

### U2 AgentSwitch
`useAgentSwitch({ sessionID: () => string, promptEmpty: () => boolean })`
- A digit key while the prompt is focused **and empty** → switch to tab N. Digits type normally when the prompt has text.
- Alt+0..9 → switch from anywhere.
- Alt+A → `View.show("architecture")`. In the architecture tab, Esc or `0` → `View.show("chat")`.
- Esc while the viewed agent is busy → confirm "Stop <agent>? (y/n)" → `session.abort`. Esc on an idle subagent tab → back to tab 0.
- `X` on an empty prompt → stop all busy agents (y/n).
- Communication:
  - reads `PromptRef.empty` to know whether the prompt is empty. This is a new getter on the existing `PromptRef` that reads the textarea's live text. The spike showed `PromptRef.current.input` updates too late when you type fast (`a1` jumped to agent 1).
  - registers keymap layers with a priority above the textarea, enabled only when the prompt is empty;
  - navigates through the route and aborts through the SDK.
- Removes: `context/nav.ts`, the `nav.enter` command, and the `nav_enter` keybind.
- Esc while the prompt is focused uses the prompt's existing `session.interrupt` command (bound to Esc again), changed from "press twice" to "confirm (y/n), then abort". That command already defers to autocomplete and shell mode.
- AgentSwitch's own Esc layer handles only the no-text-box case (subagent tabs, prompt hidden): busy → confirm and stop; idle subagent → tab 0.

### U3 StatusBar
`<StatusBar sessionID />`
- Shows plan progress and ETA (`Derive.eta`), Σ tokens and cache (`Derive.usage` / `cacheHit`), and the current agent, tier and model.
- Shows key hints taken from the actual bindings (`useCommandShortcut`).
- Replaces the prompt's footer hint row. The AgentsPanel is removed and the old right sidebar no longer opens automatically, so the chat is full width. It still opens on explicit toggle (`ctrl+x b`) for LSP, MCP and context info.

### U4 CompactTranscript
`compact(part: ToolPart): { icon: string; text: string }` (pure). Examples:
- `edit`/`write`/`apply_patch` → `✎ edited <file> +a −d`
- `bash` → `⏵ <command> (<duration>)`
- `read`/`grep`/`glob` → `· read <file>`
- `task` → `⇢ <agent>: <description>`

When compact mode is on (the default in AIOven), the session route renders tool parts as one line each using `compact()`. No diffs, no code blocks from tools, and thinking is hidden. Pressing `ctrl+o` toggles full detail for the current view.
- Built as follows: the `ToolPart` / `ReasoningPart` wrappers pick `CompactToolLine` or the full renderer reactively. The keybind is `session_toggle_compact` (ctrl+o) and the setting is stored in kv as `aioven_compact`. Clicking a task line opens that subagent's tab.

## Communication

```
SDK event stream ─▶ sync store ─┬─▶ Derive ─▶ AgentTabs, StatusBar
                                │   View (U8) ◀─ AgentSwitch, AgentTabs ─▶ session route picks transcript | ArchitectureView
                                ├─▶ session_diff + todos ─▶ PlanModel.assign/status ─▶ ArchitectureView
                                └─▶ message parts ─▶ compact() ─▶ CompactTranscript
sdk.client.file.read(plan.md) ─▶ PlanModel.parsePlan ─▶ ArchitectureView
keys ─▶ AgentSwitch ─▶ route.navigate | DialogConfirm ─▶ session.abort
```
- All new UI components are read-only views over the sync store; only AgentSwitch acts, through the route and the SDK.
- Pure functions (Derive, PlanModel, compact) hold all logic and are unit-tested; the components only wire them up.

## Open risks
- U2: digits must beat the managed textarea only while the prompt is empty. Spike this first; fallback is the leader key `ctrl+x <n>`.
- U5: a plan that doesn't follow the U7 format gives an empty component list. The view then degrades to the files-only list, with no error.

## Skill exclusion (2026-10-08)

The user removed skills they don't use. Repo skills and test fixtures were deleted outright: rtl-aware-development, plus the agents-sdk/cloudflare fixtures, which were replaced by neutral `sample-a`/`sample-b` fixtures for the URL-download test. Skills synced from Claude.ai can't be deleted locally because sync brings them back, so they are excluded instead.

### SkillExclude
```ts
// packages/core/src/aioven.ts
const EXCLUDED_SKILLS: readonly string[]            // ["morning", "import-memory"]
// packages/core/src/v1/config/skills.ts
skills?: { paths?; urls?; exclude?: string[] }      // extra names to exclude
// packages/opencode/src/skill/index.ts
function excluded(name: string, configExclude?: readonly string[]): boolean
```
The built-in `customize-opencode` skill was deleted from code. That covers its core plugin registration (`core/src/plugin/skill.ts`, `internal.ts`), the opencode loader's built-in entry, its markdown body, and its test.

Communication: the skill loader calls `excluded()` for every discovered skill, whatever its source (project, `~/.claude`, `~/.agents`, config paths, URLs). An excluded skill is never registered, so it isn't listed to the model and the `skill` tool can't load it.

---

# Confirmed todo list (2026-10-08)

| # | Todo |
|---|---|
| T1 | Brand + blue logo everywhere |
| T2 | `aioven` command |
| T3 | Rename plan→recipe, build→bake |
| T4 | Themed names |
| T5 | Framework decided: Ratatui (Rust) |
| T6 | Architecture of the Ratatui client |
| T7 | Build the Ratatui client |
| T8 | Final test run + commit |

The user confirmed the list and asked for everything to be worked through to the end. No tests run until T8.

## T1 Brand

Responsibility: one source for the product name, command, glyphs and logo colours. Every splash, logo and exit screen renders through it. No OpenCode branding remains on any start or exit path.

```ts
// packages/tui/src/brand.ts   (the CLI reaches it through packages/opencode/src/cli/logo.ts)
export const Brand: {
  name: "AIOven"
  command: "aioven"
  glyphs: { left: string[]; right: string[] }   // "ai" | "oven", 4 rows, marks _ ^ ~ as today
  mark: string[]                                 // compact 3-row "ai" badge (run-mode splash)
  colors: {                                      // forced, theme-independent
    left: RGB; right: RGB; leftShadow: RGB; rightShadow: RGB   // aqua #22d3ee, blue #3b82f6, #0e4a5a, #1e3a8a
  }
}
export function ansiLogo(pad?: string, truecolor?: boolean): string[]   // shared by CLI help/errors and the TUI exit epilogue
```

| Consumer | Change |
|---|---|
| `tui/src/component/logo.tsx` (home screen) | Brand glyphs + Brand colours instead of theme `textMuted`/`text` |
| `tui/src/util/presentation.ts` (TUI exit screen) | Its own copy of the OpenCode logo is deleted. It now uses `ansiLogo()` and `Continue  aioven -s <id>` |
| `opencode/src/cli/ui.ts` `logo()` (help, errors, upgrade, uninstall, web) | Uses `ansiLogo()`: truecolor blue, plain text when not a TTY |
| `opencode/src/cli/cmd/run/splash.ts` (run-mode entry + exit) | `go` glyphs → `Brand.mark`, "OpenCode" → `Brand.name`, colours from `Brand.colors`, `opencode --mini -s` → `aioven --mini -s` |
| `opencode/src/cli/cmd/run/theme.ts` | `splashTheme()` returns Brand colours |
| `tui/src/component/bg-pulse*.tsx`, `dialog-retry-action.tsx` | OpenCode Go upsell art deleted; the retry dialog always uses the plain layout |
| `tui/src/logo.ts` | `go` glyphs deleted; `logo` re-exported from Brand for compatibility |

## T2 `aioven` command

| Component | Interface | Notes |
|---|---|---|
| Launcher `bin/aioven` (repo root, sh) | `aioven [args…]` → `bun run <repo>/packages/opencode/src/index.ts [args…]` | Keeps the caller's working directory, so the project is wherever you run it. Linked into `~/.local/bin/aioven`, which is on PATH. |
| Package bin | `packages/opencode/package.json` `bin.aioven` → `./bin/opencode` | For installed builds. `opencode` stays as an alias. |
| CLI name | yargs `scriptName(Brand.command)`; `index.ts` `show()` checks the `Brand.command + " "` prefix | Help and usage say `aioven …` |
| Resume hints | `Brand.command` in the TUI exit screen and run-mode exit badge (T1) | `aioven -s <id>` |

## T3 + T4 Rename (agents and UI words)

Rename map: real IDs, used everywhere, and the old names stop working.

| Old | New | Kind |
|---|---|---|
| build | **bake** | main agent (default) |
| plan | **recipe** | main agent (plan mode) |
| explore | **pantry** | subagent |
| review | **taster** | subagent |
| test-runner | **thermometer** | subagent |
| research | **cookbook** | subagent |

UI words, in the TUI (Derive `label()`, tabs, architecture/blueprint view, plan approval):

| Old | New |
|---|---|
| plan approved / plan ready | recipe ready |
| done ✓ | baked |
| error ✗ | burnt |
| interrupted ■ | pulled out |
| waiting for model | preheating |
| architecture tab | blueprint |

Interface impact:
- **Agent registries:** v1 `agent/agent.ts` and v2 `core/src/plugin/agent.ts` + `core/src/agent.ts` (`defaultID` → `bake`).
- **Defaults:** `AIOvenDefaults.AGENTS` keys, the `default_agent` fallback (`bake`), the ACP default mode, and the run-mode label default.
- **Mode switch:** `tool/plan.ts` switches to `bake`; `session/reminders.ts` checks `recipe`/`bake`; the TUI switches its local agent on `plan_exit`/`plan_enter`.
- **TodoNudge:** only nudges `bake`.
- **Prompts:** `build-switch.txt`, `plan-mode.txt`, `task.txt` and the subagent descriptions use the new names.
- **Unchanged on purpose:**
  - tool IDs `plan_enter`/`plan_exit` and the `.opencode/plans/` folder (internal names, not agent names);
  - historical DB migrations (default `'build'` stays in old migration SQL);
  - the experimental v2 engine's `general` agent (v2 isn't active).
- **Tests:** references to the agents are updated to the new IDs. They are not run until T8.

---

# T6 Ratatui client (`packages/tui-rs`) — design

Decision (T5): Rust + Ratatui, Linux only. The new client is a **separate process**. It talks to the AIOven server only through its HTTP API and its live event stream, exactly like the current TUI does through the SDK. The server is unchanged.

## Screen

```
┌ 0 bake ⏳bash 12s │ 1 pantry ● 7k │ 2 taster ✓ ┆ skills: pdf · xlsx ┆ B blueprint 3/5 ┐  TopBar
├──────────────────────────────────────────────────────┬────────────────────────┤
│ FILES CHANGED                         RetryPolicy ✓  │ TOKENS BURNED          │  FilesPane │ Sidebar
│  ✎ src/retry.ts            +40 −0                    │ bake     62k ▓▓▓▓░░░░  │
│  ✎ src/client.ts            +8 −2     FetchClient ⏳  │ pantry    7k ▓░░░░░░░  │
├──────────────────────────────────────────────────────┤ Σ 69k · cache 84%     │
│ you: add retry to the fetch client                   │ RUNNING                │  ChatPane
│ bake: recipe ready, 5 steps                          │ ⏳ pantry  12s         │
│   · read client.ts   ✎ edited src/retry.ts +40 −0    │ ⏳ bun test 31s        │
│   ⏵ bun test (12s)                                   │ RECIPE ▓▓▓░░ 3/5 ~4m   │
├──────────────────────────────────────────────────────┴────────────────────────┤
│ > _                                                                            │  InputBox
│ bake M gemini-flash · ⇧⏎ send · ⇧1-9 tabs · alt+b blueprint · ctrl+o detail    │  StatusLine
└────────────────────────────────────────────────────────────────────────────────┘
```
- Files 40% / chat 60% of the middle.
- The sidebar is 30 columns.
- The input box grows to 6 lines.
- The blueprint tab replaces the middle with the component view (U5 model).
- A skill tab shows that skill's description and where it was used in the session.

## Components (Rust modules)

| # | Component | Module | Responsibility |
|---|---|---|---|
| R1 | ServerProcess | `server.rs` | Start or attach to the AIOven server, get its base URL, shut it down on exit |
| R2 | Api | `api.rs` | Typed HTTP calls for the subset of endpoints the client needs |
| R3 | EventStream | `events.rs` | `GET /event` (SSE) → typed `Event` values over a channel; reconnect with backoff |
| R4 | Store | `store.rs` | Client state; pure reducer `apply(&mut Store, Event)` |
| R5 | Derive | `derive.rs`, `plan_model.rs`, `compact.rs` | Pure ports of the TS `derive.ts` / `plan-model.ts` / `compact.ts` |
| R6 | Views | `ui/{top_bar,sidebar,files,chat,input,status,dialogs,blueprint}.rs` | Pure render functions `fn render(f, area, &Store, &UiState)` |
| R7 | Keymap | `keys.rs` | Pure `map_key(KeyEvent, &UiState) -> Option<Action>` |
| R8 | App | `app.rs`, `main.rs` | Event loop: terminal keys + store events + 1 s tick → `update(&mut App, Action)` → render |
| R9 | Markdown | `markdown.rs` | Pure `pulldown-cmark` → `ratatui::text::Text` (headings, lists, code blocks, tables) |
| R10 | Brand (Rust) | `brand.rs` | A Rust copy of T1 Brand: glyphs, blue colours, `ansi_logo()` for the exit screen. It must match `packages/tui/src/brand.ts`, which a T8 test compares. |
| R11 | Budgets | `budget.rs` | A Rust copy of `AIOvenDefaults.AGENTS` budgets and tiers, with config overrides from `/config` → `aioven.agents` |

## Interfaces

```rust
// R1
pub struct Server { pub base_url: Url, pub directory: PathBuf, child: Option<Child> }
impl Server {
    pub async fn start(project: &Path) -> Result<Server>;   // spawns `aioven serve --port 0`, reads "listening on http://…"
    pub fn attach(url: Url, project: &Path) -> Server;       // `aioven-tui --attach <url>`
}                                                            // Drop kills the child

// R2  (every call sends ?directory=<project>)
#[async_trait] pub trait Api {
    async fn agents(&self) -> Result<Vec<Agent>>;                         // GET /agent
    async fn skills(&self) -> Result<Vec<Skill>>;                         // GET /skill
    async fn config(&self) -> Result<Config>;                             // GET /config  (aioven tiers/budgets)
    async fn session_create(&self) -> Result<Session>;                    // POST /session
    async fn session(&self, id: &str) -> Result<Session>;                 // GET /session/{id}
    async fn children(&self, id: &str) -> Result<Vec<Session>>;           // GET /session/{id}/children
    async fn messages(&self, id: &str, limit: u32) -> Result<Vec<MessageWithParts>>; // GET /session/{id}/message
    async fn todos(&self, id: &str) -> Result<Vec<Todo>>;                 // GET /session/{id}/todo
    async fn diff(&self, id: &str) -> Result<Vec<FileDiff>>;              // GET /session/{id}/diff
    async fn status(&self) -> Result<HashMap<String, SessionStatus>>;     // GET /session/status
    async fn prompt(&self, id: &str, text: &str, agent: &str) -> Result<()>;  // POST /session/{id}/prompt_async
    async fn abort(&self, id: &str) -> Result<()>;                        // POST /session/{id}/abort
    async fn permission_reply(&self, req: &str, reply: Reply) -> Result<()>;  // POST /permission/{id}/reply
    async fn question_reply(&self, req: &str, answers: Vec<String>) -> Result<()>; // POST /question/{id}/reply
    async fn read_file(&self, path: &str) -> Result<Option<String>>;      // GET /file/content (recipe file)
}
// Types are hand-written serde structs for exactly these shapes. A T8 contract
// test checks them against the server's OpenAPI document (GET /doc).

// R3
pub enum Event {                                  // serde(tag = "type"), unknown types ignored
    SessionUpdated(Session), SessionStatus { session_id, status }, MessageUpdated(Message),
    PartUpdated(Part), PartRemoved{..}, TodoUpdated { session_id, todos }, SessionDiff { session_id, diff },
    PermissionAsked(PermissionRequest), PermissionReplied{..}, QuestionAsked(QuestionRequest), QuestionReplied{..},
}
pub fn subscribe(api: &HttpApi) -> mpsc::Receiver<Event>;

// R4
pub struct Store { sessions, messages, parts, status, todos, diffs, permissions, questions, agents, skills, config }
pub fn apply(store: &mut Store, event: Event);    // pure; unit-tested with recorded events

// R5  (same behaviour as the tested TS versions)
pub fn agents(store: &Store, session: &str) -> Vec<String>;      // tab order: root + ≤9 children
pub fn wait_of(store: &Store, session: &str) -> Wait;
pub fn usage(store: &Store, session: &str) -> Usage;  pub fn eta(..) -> Eta;
pub fn parse_plan(md: &str) -> Vec<Component>;  pub fn assign(..);  pub fn infer(..);  pub fn compact(part: &ToolPart) -> Line;

// R7
pub enum Action { SelectTab(u8), Blueprint, SelectSkill(usize), Send, Newline, StopAsk, StopAll,
                  Confirm(bool), PermissionReply(Reply), ToggleDetail, ScrollChat(i16), ScrollFiles(i16), Quit, Input(KeyEvent) }
pub fn map_key(key: KeyEvent, ui: &UiState) -> Option<Action>;
```

## Keys (R7)

- **Tab switching (revised while building):** Alt+0..9, plus Ctrl+0..9, which needs the kitty protocol enabled through crossterm `KeyboardEnhancementFlags`. Neither ever types into the input.
  - Shift+digits were dropped: Shift+1 *is* how `!` is typed, so the terminal reports the same key either way, and using it for tabs would break typing `!@#$%^&*(`.
- **Typing:** Enter = new line; Shift+Enter or Alt+Enter = send.
- **Esc:** if the viewed agent is busy, confirm "Stop? (y/n)"; in the blueprint or a skill tab, go back to chat.
- **Other:**
  - Ctrl+X = stop all (y/n); Alt+B = blueprint; Alt+S = next skill tab; Tab = switch the main agent bake ⇄ recipe.
  - Ctrl+O = compact ⇄ full; PgUp/PgDn = chat; Shift+PgUp/PgDn = files; Ctrl+C twice = quit.
- **Permission dialog:** y / a / n.

## Communication

```
crossterm keys ─▶ Keymap ─▶ Action ─┐
SSE /event ─▶ EventStream ─▶ Event ─┼─▶ App.update ─▶ Store (apply) ─▶ Views (render from &Store + &UiState)
1 s tick ───────────────────────────┘        │
                                             └─▶ Api (prompt, abort, replies, loads)   ◀─ ServerProcess (base URL)
```
- The App owns the Store and UiState.
- Network work runs in tokio tasks and feeds results back as messages; rendering never blocks on the network.
- On start: load agents, skills and config, create or resume the session (`-s <id>`), load its messages, todos, diff and children, then follow the event stream.

## Crates
ratatui, crossterm, tokio, reqwest (rustls), reqwest-eventsource, serde/serde_json, pulldown-cmark, tui-textarea, anyhow, url, unicode-width.

## Launch and parity
- **Binary and command:** the binary is `aioven-tui`, built from `packages/tui-rs`. `aioven` starts it when `AIOVEN_TUI=rs` is set; the old TUI is used otherwise.
- **Parity set:** chat with markdown, send/stop, agent tabs, files pane, sidebar, blueprint tab, skill tabs, permission and question dialogs, resume `-s`, model shown.
- **After parity:** the default switches to `aioven-tui`. The old interactive TUI screens are removed, but the `packages/tui` library stays: run mode, Brand and the CLI logo use it.
- **Out of the first version:** the model picker, session list, themes and plugins. Until they exist, use `aioven models` and the old TUI.

## Toolchain
Rust stable through rustup, in the user's home directory (`~/.rustup`, `~/.cargo`). No root needed.

## T7 status (2026-10-08)
- **Built:** all components R1–R11 in `packages/tui-rs`. `aioven` now starts it by default; `AIOVEN_TUI=old` starts the previous TUI.
- **Verified live:**
  - on a real 8-agent session: tabs, files, chat with markdown and compact tool lines, sidebar tokens and cache, recipe progress, Alt+digit tabs (typing `1!@` unaffected), Alt+B blueprint, Ctrl+O detail, quit with the blue exit screen;
  - with a fake model: send, the "preheating" wait label, Esc → "Stop bake?" → y → "pulled out"; the server shuts down on quit.
- **Not yet verified live:** the permission and question dialogs (they need a real model to ask). Until they are, the old OpenTUI screens stay as a fallback rather than being deleted.

---

# T9–T14 Ratatui client additions — design (confirmed todos, 2026-10-08)

| # | Change | Component(s) |
|---|---|---|
| T9 | Status line shows `ctrl+0-9 tabs` (Alt+0-9 keeps working) | `ui/status.rs` |
| T10 | Heavier look and more live indicators | `ui/mod.rs` `panel()`, `ui/sidebar.rs`, `ui/top_bar.rs`, new `ui/ticker.rs` |
| T11 | Ctrl+B moves running foreground subagents to the background | `keys.rs`, `app.rs`, `api.rs` |
| T12 | Focus the chat pane and scroll it (keys + mouse wheel) | `keys.rs`, `app.rs`, `ui/chat.rs`, `main.rs` (mouse capture) |
| T13 | `/` or `\` at the start of the input opens skill/command completion | new `complete.rs`, `ui/complete.rs`, `api.rs`, `app.rs` |
| T14 | Ctrl+P settings/command popup with a filter | new `menu.rs`, `ui/menu.rs`, `api.rs`, `app.rs` |

## Interfaces

```rust
// T10 ui/ticker.rs (pure)
pub fn spinner(now_ms: i64) -> &'static str;          // ⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ at 100 ms per frame
pub fn bar_thick(value: f64, max: f64, width: usize) -> String;   // █ / ▒ instead of ▓ / ░
// panel(): BorderType::Thick (┏━┓┃). The focused pane gets the AQUA border, the others DIM.
// Live tickers: a spinner on busy tabs and RUNNING rows; elapsed time for every busy agent (tool, model,
// subagent); "↑ <tokens>" output counter on the streaming agent, from the part.delta chars it receives.
// The app loop ticks every 100 ms while anything is busy, and every 1 s otherwise.

// T11
Action::Background                                     // Ctrl+B
Api::background(session: &str) -> Result<()>           // POST /experimental/session/{id}/background
// Enabled when the viewed root has a running `task` tool part. Notice: "moved N task(s) to background".

// T12
pub enum Focus { Input, Chat, Files }                  // UiState.focus
Action::FocusNext                                      // Ctrl+↑ = focus chat (then files); Esc/i = back to input
Action::Scroll { pane: Pane, delta: i16 }              // ↑↓ j k = 1 line, PgUp/PgDn = page, Home/End
Mouse wheel over a pane → Scroll { pane: <pane under the cursor>, ±3 }
// While focus != Input, keys don't type. The focused pane shows "· focused" in its title.

// T13 complete.rs (pure)
pub struct Entry { pub name: String, pub description: String, pub skill: bool }
pub fn trigger(input: &str) -> Option<&str>;           // Some(query) when the input starts with '/' or '\' and has no space yet
pub fn matches(entries: &[Entry], query: &str) -> Vec<&Entry>;   // prefix match first, then substring; skills marked ✦
Api::commands() -> Result<Vec<Command>>                // GET /command (skills have source "skill")
Api::command(session, name, args, agent) -> Result<()> // POST /session/{id}/command
// UI: a popup above the input. ↑↓ choose, Tab completes the name, Enter runs it with the remaining text as arguments.

// T14 menu.rs (pure)
pub enum Item { Model { provider: String, model: String }, Agent(String), Detail(bool), Blueprint, StopAll, Background, Quit }
pub fn items(store: &Store, providers: &[ProviderModels], ui: &UiState) -> Vec<(String /*label*/, Item)>;
pub fn filter(items: &[(String, Item)], query: &str) -> Vec<usize>;
Api::providers() -> Result<Vec<ProviderModels>>        // GET /provider (connected providers' models)
// The chosen model is stored in UiState.model and sent with each prompt as "model": {providerID, modelID}.
// UI: a centred popup with a filter line; ↑↓ choose, Enter applies, Esc closes.
```

## Communication
Everything stays inside the client. New server calls: `/command`, `/session/{id}/command`, `/provider` and `/experimental/session/{id}/background`, all through `Api` and spawned like the existing calls. Pure modules (`complete.rs`, `menu.rs`, `ticker.rs`, keymap additions) get unit tests.

---

# T15–T17 — design (confirmed 2026-10-08)

## T15 Caveman ultra by default
- `AIOven.terse(settings)`: the default level becomes **`ultra`**.
- The level texts are rewritten from the user's caveman skill:
  - the rules for each level;
  - Auto-Clarity: plain language for security warnings, irreversible actions, ambiguous ordering;
  - Boundaries: code, commits and PRs written normally.
- `aioven.terse: off | lite | full | ultra` still overrides.
- Applies to every agent:
  - Session-loop agents get it as the first per-request system part (unchanged, so the cache stays safe).
  - Hidden helpers (`compaction`, `title`, `summary`) get the same text appended to their agent prompt in `agent/agent.ts`.

## T16 CBSE central
```ts
// opencode/src/aioven/index.ts
function cbse(agent: string): string | undefined   // deterministic text; placed right after terse in the system parts
```
- **a) bake:** before any edit, name the components it touches, their interfaces (signatures, types, events, errors) and how they communicate.
  - If a recipe exists, implement its interfaces exactly, and say so before deviating.
  - With no recipe and more than one component affected, first write a short Components / Interfaces / Communication block.
  - Narrow interfaces; no reaching into another component's internals.
- **b) taster:** its prompt adds a conformance pass that comes before bugs. It checks:
  - signature or type drift from the recipe;
  - new public surface that isn't in the recipe;
  - cross-component internal access;
  - dependency direction.
  Findings are prefixed `[interface]`.
- **c) Recipe format (plan-mode prompt) and parsing:**
  - `### Interfaces` bullets: `` - **<Component>**: `<signature or event>` `` (one per line, several per component allowed).
  - `### Communication` bullets: `- <From> → <To>: <what> (sync|async|event)`.
  - `plan_model.rs`: `parse_interfaces(md) -> Vec<(component, signature)>`, `parse_communication(md) -> Vec<(from, to, what)>`.
  - The blueprint shows the interfaces under each component, then a COMMUNICATION section with arrows.
- **d) Ratatui layout:** the middle area is the **Blueprint pane (top, 50%)** and **Chat (bottom)**.
  - The blueprint already lists files under their components plus "other files", so the separate files pane goes away.
  - Ctrl+G toggles a full-height blueprint (T17).
  - Focus cycling becomes Input → Chat → Blueprint.

## T17 Blueprint key under tmux
tmux takes Ctrl+B as its prefix.
- Blueprint toggle = **Ctrl+G** (and Alt+B).
- Background stays on Ctrl+B (press it twice inside tmux) and is also in the Ctrl+P menu.
- The hints and AIOVEN.md show Ctrl+G.

---

# T18–T23 — design (2026-10-09, awaiting confirmation)

Finding: "submitting does nothing" was a hidden error. The Gemini free tier is exhausted (20 requests/day), so the server keeps retrying, and the Ratatui client only showed a truncated `retr…`. The prompt itself was sent.

| # | Todo | Components |
|---|---|---|
| T18 | Make failures visible | `derive.rs` (wait labels), `ui/chat.rs`, `ui/status.rs`, `app.rs` |
| T19 | Restore Connect (provider login) in the new TUI | `api.rs`, new `connect.rs` (pure flow), `ui/connect.rs`, `menu.rs`, `app.rs` |
| T20 | Permission + question dialogs, verified end-to-end | `store.rs` (initial load), `api.rs`, `ui/dialogs.rs`, fake-model check |
| T21 | Remove the old UI | `bin/aioven`, `opencode/src/cli/cmd/tui.ts`, `attach.ts`, `cli/tui/*`, dead `packages/tui` app code |
| T22 | Usage stats page (since install, daily heat map, month, year) | server: new `aioven/usage.ts` + route `GET /experimental/aioven/usage`; client: `api.rs`, `usage.rs` (pure), `ui/usage.rs`, tab `U` |

## Interfaces

```rust
// T18
Wait::Retry { attempt, next, message }                // message kept; label: "retry #2 in 20h34m: quota exceeded…"
// chat: a red "↻ retrying: <message>" / "✗ burnt: <message>" line under the turn; status line shows the full message
// as a notice. Errors from Api calls already go to the notice; they now also stay visible for 10 s.

// T19 connect.rs — pure state machine
enum Step { Pick { query }, Method { provider, methods }, Prompts { .. }, ApiKey { provider, key }, Oauth { provider, url, instructions, code }, Done }
Api::auth_methods() -> HashMap<provider, Vec<AuthMethod>>        // GET /provider/auth
Api::set_api_key(provider, key)                                    // PUT /auth/{provider}  { type: "api", key }
Api::oauth_authorize(provider, method, inputs) -> Authorization    // POST /provider/{id}/oauth/authorize → { url, method: auto|code, instructions }
Api::oauth_callback(provider, method, code?)                       // POST /provider/{id}/oauth/callback
// UI: Ctrl+P → "Connect provider…" (and /connect in the input). It is a list → method → key or OAuth URL (+ code) popup.
// After success the models are reloaded.

// T20
Api::permissions() / Api::questions()                              // GET /permission, GET /question (requests pending before the client started)
// The dialogs already exist and are driven by the store. T20 adds the initial load plus a live check: a fake model asks for
// bash (permission) and calls the question tool; y/a/n and ↑↓⏎ are verified.

// T21
// `aioven [project]`, `aioven -s`, `aioven attach <url>` → exec aioven-tui (the CLI default command, the `tui` and `attach`
// commands, and the launcher; AIOVEN_TUI=old is removed).
// The OpenTUI interactive app (packages/tui: app.tsx, routes/, interactive-only components and plugin slots) is deleted.
// Kept: the parts run mode and the CLI import (config, keymap, brand, logo, theme, util, editor, spinner, prompt display).
// An import-graph check decides each file.

// T22 server: opencode/src/aioven/usage.ts
type Day = { day: "YYYY-MM-DD"; input; output; reasoning; cache_read; cache_write; cost; messages }
function daily(db, from?: Date): Day[]          // SQL aggregate over assistant messages, grouped by local day
GET /experimental/aioven/usage?from=YYYY-MM-DD → { days: Day[], first: "YYYY-MM-DD" }   // first = installation (first message)
// client usage.rs (pure): year_grid(days, year) -> 53×7 cells with level 0–4; month_totals(days); summary(days)
// ui/usage.rs: "U usage" tab
//   - GitHub-style heat map of tokens per day for the selected year (←/→ year);
//   - a bar per month (12 rows);
//   - totals since install: tokens, cost, messages, busiest day.
// Ctrl+U or Ctrl+P → "Usage stats".
```

## T18–T22 status (2026-10-09)
- **T18 errors:** verified live with a fake 429 provider. The chat shows `↻ retry #3 in 9s: You exceeded your current quota…`, and so does the status line.
- **T20 dialogs:** verified live with a fake tool-calling model: permission (y) → bash ran → question (↓⏎) → final answer. Pending requests now load at start and on reconnect.
- **T19 connect:** the API-key flow was verified live in an isolated data directory. OAuth is unit-tested only; a live run would open a browser.
- **T22 usage:** the endpoint was verified on a copy of real history (138 active days since 2026-04-28), and the heat-map page rendered.
- **T21 old UI removed:**
  - The CLI default command and `attach` start the Ratatui client (`cli/cmd/rust-tui.ts`); `--mini` and `run` stay on run mode.
  - Deleted: 138 unreachable `packages/tui` files (found by an import-graph walk from the remaining users), `cli/tui/*`, `plugin/tui/*`, and the tests that only covered them.
  - Kept: the library run mode needs (brand, config, keymap, theme, prompt, util, editor, parsers-config).
