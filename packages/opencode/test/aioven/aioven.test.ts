import { describe, expect, test } from "bun:test"
import { AIOven } from "../../src/aioven"
import { AIOvenDefaults } from "@opencode-ai/core/aioven"

describe("AIOven.agent", () => {
  test("defaults to the smallest sufficient tier without a model when tiers are unset", () => {
    expect(AIOven.agent(undefined, "pantry")).toEqual({ tier: "small", budget: 50_000, model: undefined, variant: "low" })
    expect(AIOven.agent(undefined, "unknown")).toBeUndefined()
  })

  test("maps tiers to models and honours per-agent overrides", () => {
    const settings = {
      tiers: { small: "github-copilot/gpt-5-mini", large: "github-copilot/claude-opus-4.5" },
      agents: { explore: { tier: "large" as const, budget: 9_000 } },
    }
    expect(AIOven.agent(settings, "pantry")).toEqual({
      tier: "large",
      budget: 9_000,
      model: "github-copilot/claude-opus-4.5",
      variant: "medium",
    })
    expect(AIOven.agent(settings, "thermometer")?.model).toBe("github-copilot/gpt-5-mini")
    expect(AIOven.agent(settings, "bake")?.model).toBeUndefined()
  })
})

describe("AIOven.terse", () => {
  test("full by default, off disables, levels differ", () => {
    expect(AIOven.terse(undefined)).toContain("terse")
    expect(AIOven.terse({ terse: "off" })).toBeUndefined()
    expect(AIOven.terse({ terse: "lite" })).not.toEqual(AIOven.terse({ terse: "ultra" }))
  })

  test("is deterministic so the system prompt prefix stays cacheable", () => {
    expect(AIOven.terse({ terse: "full" })).toBe(AIOven.terse({ terse: "full" }))
    expect(AIOven.terse({ terse: "full" })).not.toMatch(/\d{4}-\d{2}-\d{2}/)
  })
})

describe("AIOvenDefaults.budget", () => {
  test("config override, then agent default, then fallback", () => {
    expect(AIOvenDefaults.budget({ agents: { build: { budget: 1 } } }, "bake", false)).toBe(1)
    expect(AIOvenDefaults.budget(undefined, "taster", true)).toBe(80_000)
    expect(AIOvenDefaults.budget(undefined, "custom", true)).toBe(50_000)
    expect(AIOvenDefaults.budget(undefined, "custom", false)).toBe(200_000)
  })
})
