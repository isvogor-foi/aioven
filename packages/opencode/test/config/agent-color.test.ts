import { expect } from "bun:test"
import { LayerNode } from "@opencode-ai/core/effect/layer-node"
import { Effect } from "effect"
import { Config } from "@/config/config"
import { Agent as AgentSvc } from "../../src/agent/agent"
import { testEffect } from "../lib/effect"

const it = testEffect(LayerNode.compile(LayerNode.group([Config.node, AgentSvc.node])))

it.instance(
  "agent color parsed from project config",
  () =>
    Effect.gen(function* () {
      const cfg = yield* Config.use.get()
      expect(cfg.agent?.["bake"]?.color).toBe("#FFA500")
      expect(cfg.agent?.["recipe"]?.color).toBe("primary")
    }),
  {
    git: true,
    config: {
      agent: {
        bake: { color: "#FFA500" },
        recipe: { color: "primary" },
      },
    },
  },
)

it.instance(
  "Agent.get includes color from config",
  () =>
    Effect.gen(function* () {
      const plan = yield* AgentSvc.use.get("recipe")
      expect(plan?.color).toBe("#A855F7")
      const build = yield* AgentSvc.use.get("bake")
      expect(build?.color).toBe("accent")
    }),
  {
    git: true,
    config: {
      agent: {
        recipe: { color: "#A855F7" },
        bake: { color: "accent" },
      },
    },
  },
)
