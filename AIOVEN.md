# AIOven

AIOven is a fork of OpenCode that works more like Claude Code: one main agent writes the code, small read-only subagents help it, and a right-hand panel shows what every agent is doing and how many tokens it has used.

Run it with `aioven [project-path]` (or `bun dev` inside this repo).

## Screen (Ratatui client, default)

`aioven` starts the Ratatui client (`packages/tui-rs`). `AIOVEN_TUI=old aioven` starts the previous OpenTUI interface.

- **Top bar:** one tab per agent (`0` = main, `1`–`9` = subagents) with live status, then the skills used in the session, then `B blueprint`.
- **Middle:** changed files on top (with their recipe component), chat of the selected tab below. Tool calls show as one line each.
- **Sidebar:** tokens burned per agent against its budget, total and cache hit, running tasks, recipe progress and ETA.
- **Bottom:** the input box (shows the current main agent) and a status line.

## Keys

| Key | Action |
|---|---|
| Enter | new line |
| Shift+Enter / Alt+Enter | send |
| Ctrl+0–9 (or Alt+0–9) | open tab N; never types into the input |
| `/` or `\` at line start | skills (✦) and commands; ↑↓ choose, Tab complete, Enter run, Esc close |
| Ctrl+P | menu: model for next prompts, bake/recipe, transcript detail, blueprint, background, stop all, quit (type to filter) |
| Ctrl+B | move running foreground subagents to the background |
| Ctrl+↑ | focus chat, then files (↑↓ / j k / PgUp PgDn / g G scroll; Esc or i back to typing) |
| Mouse wheel | scroll chat or files under the pointer |
| Alt+B | blueprint tab (Esc returns) |
| Alt+S | next skill tab |
| Esc | stop the agent you're viewing (asks y/n); on an idle subagent tab, back to main |
| Ctrl+X | stop all busy agents (asks y/n) |
| Tab | switch the main agent bake ⇄ recipe |
| Ctrl+O | compact ⇄ full transcript |
| y / a / n | permission prompt: once / always / reject |
| Ctrl+C twice | quit |

Status line: `bake M model` = main agent, its tier (S/M/L = small/medium/large) and model. `B blueprint` = the blueprint tab and its key letter.

Shift+digits are not tab keys, because Shift+1 is how you type `!`.

Under tmux, Shift+Enter needs extended keys:

```
set -s extended-keys on
set -as terminal-features '*:extkeys'
```

If your terminal can't tell Shift+Enter apart from Enter, use Alt+Enter to send.

## Agents

| Agent | Kind | Default tier | Job |
|---|---|---|---|
| bake | main | medium | does the work; the only agent that edits code |
| recipe | main | medium | read-only planning; when the recipe is ready, asks before switching to bake |
| pantry | subagent | small | searches the codebase |
| taster | subagent | medium | reviews the diff (read-only, `git diff`/`log`/`show`/`status` only) |
| thermometer | subagent | small | runs builds and tests, reports only failures |
| cookbook | subagent | small | looks up docs and issues on the web |

## Built-in behaviour

- **Recipe mode** (planning) is on by default. Recipes must define components, interfaces and communication before any implementation steps. When you approve a recipe ("recipe ready"), it switches to bake, which turns the steps into todos.
- **Todo nudge:** if bake stops while todos are still open, it gets one short "continue" prompt. It gets at most 2 nudges without progress.
- **Background subagents** are on by default.
- **Tools:** the `lsp` tool is on by default. `ast_grep` does structural search and rewrite (bundled `@ast-grep/cli`).
- Nested `AGENTS.md` files are attached automatically the first time a file in that directory is read.

## Skills

Removed:
- `rtl-aware-development` (repo) and the built-in `customize-opencode`.
- `morning` and `import-memory` are excluded. They are synced from Claude.ai, so they are skipped rather than deleted.

To skip more skills, add their names to `"skills": { "exclude": ["name"] }`.

## Config (`opencode.json`)

```jsonc
{
  "aioven": {
    // Model for each tier. Without this, agents use the session model.
    "tiers": {
      "small": "github-copilot/gpt-5-mini",
      "medium": "github-copilot/claude-sonnet-4.5",
      "large": "github-copilot/claude-opus-4.5"
    },
    // How compactly agents write and think: off | lite | full | ultra (default: full)
    "terse": "full",
    // Per-agent tier and soft token budget (the panel's token bar)
    "agents": {
      "pantry": { "tier": "small", "budget": 50000 },
      "bake": { "budget": 300000 }
    }
  }
}
```

- An explicit `agent.<name>.model` in your config always wins over the tier.
- Each tier also sets a default reasoning effort (small/medium = low, large = medium). It only applies when the model supports it.
- `aioven.tiers.small` is also used for session titles and summaries (unless `small_model` is set).
- Budgets are soft: the panel's token bar turns yellow at 80% and red above 100%, but work never stops.
