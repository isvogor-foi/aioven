# Keeping up with OpenCode

AIOven is a deep fork, so **merge** upstream; don't rebase, because a rebase would replay every rename against new upstream code. Remotes: `origin` = the AIOven fork (branch `main`), `upstream` = `anomalyco/opencode` (only its `dev` is fetched).

```sh
git fetch upstream dev
git switch -c sync-$(date +%F) main
git merge upstream/dev            # resolve conflicts (see hot spots)
bun install && (cd packages/opencode && bun run typecheck)
cargo test --manifest-path packages/tui-rs/Cargo.toml
(cd packages/opencode && bun test)   # then a live smoke run: aioven in a scratch repo
```

## Hot spots (where conflicts land)

| Area | Why | Resolution |
|---|---|---|
| `packages/tui/**`, `cli/cmd/tui.ts`, `cli/cmd/attach.ts`, `plugin/tui/**` | The old OpenTUI interface was removed (138 files). | Upstream edits to deleted files appear as modify/delete conflicts: keep them deleted (`git rm`). New upstream TUI files that nothing imports can be deleted too. |
| `agent/agent.ts`, `agent/prompt/*.txt` | Agents renamed (bake, recipe, pantry, taster, thermometer, cookbook); prompt files renamed to match. | Take upstream logic changes and keep the AIOven names. |
| `session/prompt.ts`, `session/reminders.ts`, `session/system.ts` | Todo nudge, terse mode (caveman), component-design reminders. | Keep both sides; re-check that the `AIOven.*` calls are still reached. |
| `core/src/v1/config/config.ts` | `aioven` config block. | Keep the block. |
| `server/routes/instance/httpapi/*/experimental.ts` | AIOven endpoints (`/experimental/aioven/usage`, `…/session/:id/diff`, background). | Keep them; follow any new endpoint API style. |
| Server API shapes used by `packages/tui-rs` | The Rust client decodes JSON by hand. | After a merge, diff `packages/sdk/js/src/v2/gen/types.gen.ts`; if Session, Message, Part, FileDiff, the event types or prompt bodies changed, update `tui-rs/src/types.rs` and `api.rs`. |

## Trial merge (2026-10-09)

`upstream/dev` was 14 commits ahead. The trial merge in a throwaway worktree was **clean**: 69 files (67 modified, 2 added), no conflicts, and none in the hot spots above. It was not merged. Run the steps above when you want it.

One upstream behaviour change was already handled: `GET /session/{id}/diff` returns `[]` without `messageID`, because diffs are now stored per turn. The blueprint uses `/experimental/aioven/session/{id}/diff` instead.
