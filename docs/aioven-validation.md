# Validating AIOven with real models

These checks need your accounts and paid model usage, so they are left for you to run. Everything else was verified with fake models; see `ARCHITECTURE-AIOVEN.md`.

## P1: real-model run (about 15 minutes)

1. `aioven` in a small scratch repo, then `/connect` → GitHub Copilot (browser login) or any other provider.
2. Ctrl+P → set the tier models, for example small = a mini model and large = your best model.
3. Shift+Tab to **recipe**: "add a CLI flag --json to <tool>, with tests". Check:
   - the recipe has Components / Interfaces / Communication sections;
   - the blueprint (Ctrl+G) shows them;
   - the answer is in caveman style.
4. Approve; it switches to **bake**. Check:
   - todos name their components, and the blueprint ticks them off;
   - `pantry` / `taster` / `thermometer` appear as tabs;
   - the todo nudge keeps the agent going until the todos are done;
   - Ctrl+U shows the tokens.
5. Note anything that is wrong.

## P2: benchmark (about 2 hours of model time)

Use the same 5 tasks and the same model for each harness: vanilla OpenCode, oh-my-opencode, opencode-slim and AIOven.

| Record | How |
|---|---|
| tokens (input, output, cache) | AIOven: Ctrl+U or the sidebar. OpenCode: `opencode stats`. |
| wall time | stopwatch, or session timestamps |
| success | the task's own tests pass, without manual fixes |
| human interventions | count |

Suggested tasks: (1) a bug fix with a failing test; (2) a small feature across 3 files; (3) a refactor that renames a module; (4) adding tests for an untested function; (5) "explain how X works" (read-only).

Pass criterion: AIOven uses at least 30% fewer tokens than vanilla OpenCode at an equal success rate.
