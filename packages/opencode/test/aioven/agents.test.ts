import { describe, expect, test } from "bun:test"
import { AIOvenAgents } from "../../src/aioven/agents"

const agents = [
  { name: "bake", mode: "primary", model: { providerID: "gh", modelID: "big" } },
  { name: "pantry", mode: "subagent", model: { providerID: "gh", modelID: "mini" } },
  { name: "taster", mode: "subagent" },
  { name: "title", mode: "primary", hidden: true },
  { name: "custom", mode: "subagent" },
]

describe("AIOvenAgents.list", () => {
  test("lists the AIOven agents in order with size, model and where the model comes from", () => {
    const list = AIOvenAgents.list({
      settings: { tiers: { small: "gh/mini" }, agents: { bake: { tier: "large" } } },
      agents,
      defaultModel: "google/flash",
    })
    expect(list.map((a) => a.name)).toEqual(["bake", "pantry", "taster"])
    expect(list[0]).toEqual({ name: "bake", mode: "primary", recommended: "medium", tier: "large", model: "gh/big", source: "agent" })
    expect(list[1]).toMatchObject({ recommended: "small", tier: "small", model: "gh/mini", source: "tier" })
    expect(list[2]).toMatchObject({ mode: "subagent", recommended: "medium", model: "google/flash", source: "default" })
  })
})
