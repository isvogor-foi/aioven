export * as AIOvenAgents from "./agents"

import type { ConfigV1 } from "@opencode-ai/core/v1/config/config"
import { AIOvenDefaults } from "@opencode-ai/core/aioven"

type Settings = NonNullable<ConfigV1.Info["aioven"]>
type Agent = { name: string; mode: string; hidden?: boolean; model?: { providerID: string; modelID: string } }

// T27: the 6 AIOven agents with their recommended size, chosen size and resolved model (right bar + settings).
export function list(input: { settings?: Settings; agents: Agent[]; defaultModel?: string }) {
  return input.agents
    .filter((agent) => !agent.hidden && AIOvenDefaults.AGENTS[agent.name])
    .map((agent) => {
      const recommended = AIOvenDefaults.AGENTS[agent.name].tier
      const tier = AIOvenDefaults.tier(input.settings, agent.name) ?? recommended
      const own = agent.model ? `${agent.model.providerID}/${agent.model.modelID}` : undefined
      // subagents without a model run on their parent's model, which is the default model for a new session
      return {
        name: agent.name,
        mode: agent.mode === "subagent" ? ("subagent" as const) : ("primary" as const),
        recommended,
        tier,
        model: own ?? input.defaultModel,
        source: !own ? ("default" as const) : own === input.settings?.tiers?.[tier] ? ("tier" as const) : ("agent" as const),
      }
    })
    .toSorted((a, b) => ORDER.indexOf(a.name) - ORDER.indexOf(b.name))
}

const ORDER = ["bake", "recipe", "pantry", "taster", "thermometer", "cookbook"]
