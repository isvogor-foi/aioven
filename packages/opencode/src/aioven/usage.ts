export * as AIOvenUsage from "./usage"

import { Effect } from "effect"
import { sql } from "drizzle-orm"
import { Database } from "@opencode-ai/core/database/database"

// T22: token usage per local day across every session (since installation).
export type Day = {
  day: string
  input: number
  output: number
  reasoning: number
  cacheRead: number
  cacheWrite: number
  cost: number
  messages: number
}

type Row = {
  day: string
  input: number | null
  output: number | null
  reasoning: number | null
  cache_read: number | null
  cache_write: number | null
  cost: number | null
  messages: number
}

export const daily = Effect.fn("AIOvenUsage.daily")(function* (from?: string) {
  const { db } = yield* Database.Service
  const since = from ? new Date(`${from}T00:00:00`).getTime() : 0
  const rows = yield* db
    .all<Row>(
      sql`SELECT date(time_created / 1000, 'unixepoch', 'localtime') AS day,
                 sum(json_extract(data, '$.tokens.input')) AS input,
                 sum(json_extract(data, '$.tokens.output')) AS output,
                 sum(json_extract(data, '$.tokens.reasoning')) AS reasoning,
                 sum(json_extract(data, '$.tokens.cache.read')) AS cache_read,
                 sum(json_extract(data, '$.tokens.cache.write')) AS cache_write,
                 sum(json_extract(data, '$.cost')) AS cost,
                 count(*) AS messages
            FROM message
           WHERE json_extract(data, '$.role') = 'assistant' AND time_created >= ${since}
           GROUP BY day
           ORDER BY day`,
    )
    .pipe(Effect.orDie)
  const first = yield* db
    .get<{ day: string | null }>(sql`SELECT date(min(time_created) / 1000, 'unixepoch', 'localtime') AS day FROM message`)
    .pipe(Effect.orDie)
  const days: Day[] = rows.map((r) => ({
    day: r.day,
    input: r.input ?? 0,
    output: r.output ?? 0,
    reasoning: r.reasoning ?? 0,
    cacheRead: r.cache_read ?? 0,
    cacheWrite: r.cache_write ?? 0,
    cost: r.cost ?? 0,
    messages: r.messages,
  }))
  return { first: first?.day ?? undefined, days }
})
