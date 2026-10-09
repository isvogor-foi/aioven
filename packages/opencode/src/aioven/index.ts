export * as AIOven from "."

import type { ConfigV1 } from "@opencode-ai/core/v1/config/config"
import { AIOvenDefaults } from "@opencode-ai/core/aioven"

export type Tier = AIOvenDefaults.Tier
type Terse = "off" | "lite" | "full" | "ultra"
type Settings = NonNullable<ConfigV1.Info["aioven"]>

export function agent(settings: Settings | undefined, name: string) {
  const base = AIOvenDefaults.AGENTS[name]
  const override = settings?.agents?.[name]
  const tier = override?.tier ?? base?.tier
  if (!tier) return
  return {
    tier,
    budget: override?.budget ?? base?.budget,
    // a per-agent model (T29) wins over the tier's model; "" means "use the tier"
    model: override?.model || settings?.tiers?.[tier],
    variant: AIOvenDefaults.VARIANT[tier],
  }
}

const CLARITY = `Plain language (no compression) for: security warnings, irreversible actions, multi-step orders where dropped words risk misreading, or when the user asks to clarify. Resume after.
Code, file contents, commits, PRs, errors (quoted exact): write normal.`

// Caveman rules (from the user's caveman skill); default level is ultra.
const TERSE: Record<Exclude<Terse, "off">, string> = {
  lite: `# Style: caveman lite
No filler, hedging, pleasantries. Keep articles + full sentences. Professional but tight.
${CLARITY}`,
  full: `# Style: caveman
Talk terse like smart caveman. All technical substance stay, only fluff die.
Drop articles, filler (just/really/basically), pleasantries, hedging. Fragments ok. Short synonyms. Technical terms exact.
Pattern: [thing] [action] [reason]. [next step].
Subagent messages: "files:", "found:", "fail:" lines only.
${CLARITY}`,
  ultra: `# Style: caveman ultra
Max compression. Abbreviate prose words (DB/auth/config/req/res/fn/impl), strip conjunctions, arrows for causality (X → Y), one word when one word enough.
Never abbreviate code symbols, function/API names, error strings.
Reasoning: shortest path, no recap, no second-guessing.
Subagent messages: "files:", "found:", "fail:" lines only.
${CLARITY}`,
}

export function terse(settings: Settings | undefined) {
  const level = settings?.terse ?? "ultra"
  if (level === "off") return
  return TERSE[level]
}

// Component-based software engineering guidance per agent (T16). Deterministic → cache-safe.
const CBSE_BAKE = `# Engineering: components first
Before any edit: name components touched, their interfaces (signatures/types/events/errors), how they communicate.
Recipe exists (.opencode/plans/*.md) → implement its interfaces exactly; say so before deviating.
No recipe + change spans >1 component → first write short "Components / Interfaces / Communication" block, then code.
Narrow interfaces, one-way deps, no reaching into another component's internals.`

const CBSE_RECIPE = `# Engineering: components first
Recipe must define components, interfaces and communication before any implementation step (see plan format).`

export function cbse(agent: string) {
  if (agent === "bake") return CBSE_BAKE
  if (agent === "recipe") return CBSE_RECIPE
  return
}
