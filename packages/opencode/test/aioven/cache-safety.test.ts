import { describe, expect, test } from "bun:test"
import path from "path"
import { Glob } from "bun"

// Prompt text must be byte-identical between requests, or provider prompt caches miss.
const VOLATILE = /Date\.now|new Date\(|Math\.random|randomUUID|performance\.now|process\.hrtime/

const root = path.join(import.meta.dir, "../../src")

async function scan(pattern: string) {
  const files = await Array.fromAsync(new Glob(pattern).scan({ cwd: root }))
  return Promise.all(files.map(async (file) => ({ file, text: await Bun.file(path.join(root, file)).text() })))
}

describe("cache safety", () => {
  test("AIOven prompt sources contain no volatile values", async () => {
    const sources = [
      ...(await scan("aioven/index.ts")),
      ...(await scan("aioven/todo-nudge.ts")),
      ...(await scan("agent/prompt/*.txt")),
      ...(await scan("session/prompt/*.txt")),
    ]
    expect(sources.length).toBeGreaterThan(5)
    const offenders = sources.filter((s) => VOLATILE.test(s.text)).map((s) => s.file)
    expect(offenders).toEqual([])
  })

  test("terse style is the first per-request system part", async () => {
    const text = await Bun.file(path.join(root, "session/prompt.ts")).text()
    expect(text).toMatch(/const system = \[\s*\.\.\.\(terse \? \[terse\] : \[\]\),\s*\.\.\.\(cbse \? \[cbse\] : \[\]\),\s*\.\.\.env,/)
  })
})
