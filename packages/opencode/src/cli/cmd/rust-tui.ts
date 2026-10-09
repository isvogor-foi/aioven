import path from "path"
import { UI } from "@/cli/ui"

// T21: the interactive TUI is the Ratatui client (packages/tui-rs). The OpenTUI app was removed.
export function rustTuiBinary() {
  const candidates = [
    process.env.AIOVEN_TUI_BIN,
    path.resolve(import.meta.dir, "../../../../tui-rs/target/release/aioven-tui"),
    Bun.which("aioven-tui") ?? undefined,
  ]
  return candidates.find((p): p is string => !!p && Bun.file(p).size > 0)
}

export async function launchRustTui(args: string[], cwd?: string) {
  const bin = rustTuiBinary()
  if (!bin) {
    UI.error("aioven-tui is not built. Run: cargo build --release --manifest-path packages/tui-rs/Cargo.toml")
    process.exitCode = 1
    return
  }
  const proc = Bun.spawn([bin, ...args], { cwd, stdin: "inherit", stdout: "inherit", stderr: "inherit" })
  process.exitCode = await proc.exited
}
