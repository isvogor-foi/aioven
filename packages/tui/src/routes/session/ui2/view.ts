// U8 ViewState: which view the session screen shows.
import { createSignal } from "solid-js"

export type ViewName = "chat" | "architecture"

const [current, setCurrent] = createSignal<ViewName>("chat")

export const View = {
  current,
  show: (view: ViewName) => setCurrent(view),
}
