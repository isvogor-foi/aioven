export * as AIOvenDefaults from "./aioven"

export type Tier = "small" | "medium" | "large"

// Smallest tier that is good enough for each agent; budgets are soft (panel colour only).
export const AGENTS: Record<string, { tier: Tier; budget: number }> = {
  bake: { tier: "medium", budget: 200_000 },
  recipe: { tier: "medium", budget: 200_000 },
  pantry: { tier: "small", budget: 50_000 },
  taster: { tier: "medium", budget: 80_000 },
  thermometer: { tier: "small", budget: 40_000 },
  cookbook: { tier: "small", budget: 50_000 },
  // Hidden helpers: session titles and summaries are cheap work.
  title: { tier: "small", budget: 10_000 },
  summary: { tier: "small", budget: 20_000 },
}

export const FALLBACK_BUDGET = { primary: 200_000, subagent: 50_000 }

// Reasoning effort per tier, used only when the model offers that variant.
export const VARIANT: Record<Tier, string> = { small: "low", medium: "low", large: "medium" }

export function budget(
  settings: { agents?: Record<string, { budget?: number } | undefined> } | undefined,
  name: string,
  subagent: boolean,
) {
  return (
    settings?.agents?.[name]?.budget ??
    AGENTS[name]?.budget ??
    (subagent ? FALLBACK_BUDGET.subagent : FALLBACK_BUDGET.primary)
  )
}

export function tier(
  settings: { agents?: Record<string, { tier?: Tier } | undefined> } | undefined,
  name: string,
): Tier | undefined {
  return settings?.agents?.[name]?.tier ?? AGENTS[name]?.tier
}

// Skills the user removed; synced from Claude.ai, so excluded instead of deleted.
export const EXCLUDED_SKILLS: readonly string[] = ["morning", "import-memory"]
