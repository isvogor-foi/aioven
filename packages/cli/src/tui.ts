import path from "path"
import { Effect } from "effect"

// AIOven: the interactive TUI is the Ratatui client (packages/tui-rs); the OpenTUI app was removed.
// It attaches to the daemon's server. Auth headers are not forwarded: the daemon is local.
export function runTui(transport: { url: string; headers: RequestInit["headers"] }) {
  return Effect.promise(async () => {
    const bin = [
      process.env.AIOVEN_TUI_BIN,
      path.resolve(import.meta.dir, "../../tui-rs/target/release/aioven-tui"),
      Bun.which("aioven-tui") ?? undefined,
    ].find((p): p is string => !!p && Bun.file(p).size > 0)
    if (!bin) {
      console.error("aioven-tui is not built. Run: cargo build --release --manifest-path packages/tui-rs/Cargo.toml")
      process.exitCode = 1
      return
    }
    const proc = Bun.spawn([bin, "--attach", transport.url], { stdin: "inherit", stdout: "inherit", stderr: "inherit" })
    process.exitCode = await proc.exited
  })
}
