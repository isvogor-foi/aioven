import { RGBA, TextAttributes } from "@opentui/core"
import { For, type JSX } from "solid-js"
import { Brand, type RGB } from "../brand"

const rgba = (c: RGB) => RGBA.fromInts(c[0], c[1], c[2])

// Home-screen logo in forced blue shades (Brand), independent of the theme.
export function Logo() {
  const renderLine = (line: string, fg: RGBA, shadow: RGBA, bold: boolean): JSX.Element[] => {
    const attrs = bold ? TextAttributes.BOLD : undefined
    return Array.from(line).map((char) => {
      if (char === "_") {
        return (
          <text fg={fg} bg={shadow} attributes={attrs} selectable={false}>
            {" "}
          </text>
        )
      }
      if (char === "^") {
        return (
          <text fg={fg} bg={shadow} attributes={attrs} selectable={false}>
            ▀
          </text>
        )
      }
      if (char === "~") {
        return (
          <text fg={shadow} attributes={attrs} selectable={false}>
            ▀
          </text>
        )
      }
      if (char === ",") {
        return (
          <text fg={shadow} attributes={attrs} selectable={false}>
            ▄
          </text>
        )
      }
      return (
        <text fg={fg} attributes={attrs} selectable={false}>
          {char}
        </text>
      )
    })
  }

  return (
    <box>
      <For each={Brand.glyphs.left}>
        {(line, index) => (
          <box flexDirection="row" gap={1}>
            <box flexDirection="row">
              {renderLine(line, rgba(Brand.colors.left), rgba(Brand.colors.leftShadow), false)}
            </box>
            <box flexDirection="row">
              {renderLine(Brand.glyphs.right[index()], rgba(Brand.colors.right), rgba(Brand.colors.rightShadow), true)}
            </box>
          </box>
        )}
      </For>
    </box>
  )
}
