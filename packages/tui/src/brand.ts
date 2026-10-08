// T1 Brand: single source for AIOven's name, command, glyphs and logo colours (see ARCHITECTURE-AIOVEN.md).

export type RGB = readonly [number, number, number]

// Glyph marks: _ = full shadow cell, ^ = top half on shadow, ~ = shadow top half.
const glyphs = {
  left: ["     ▀", "▀▀▀█ █", "█^^█ █", "▀▀▀▀ ▀"],
  right: ["                   ", "█▀▀█ █  █ █▀▀█ █▀▀▄", "█__█ ▀▄▄▀ █^^^ █__█", "▀▀▀▀  ▀▀  ▀▀▀▀ ▀~~▀"],
}

export const Brand = {
  name: "AIOven",
  command: "aioven",
  glyphs,
  // Compact badge for run-mode splashes: the "ai" part.
  mark: glyphs.left.slice(1),
  // Forced blue shades, independent of the theme.
  colors: {
    left: [0x22, 0xd3, 0xee] as RGB,
    right: [0x3b, 0x82, 0xf6] as RGB,
    leftShadow: [0x0e, 0x4a, 0x5a] as RGB,
    rightShadow: [0x1e, 0x3a, 0x8a] as RGB,
  },
} as const

const fg = (c: RGB) => `\x1b[38;2;${c[0]};${c[1]};${c[2]}m`
const bg = (c: RGB) => `\x1b[48;2;${c[0]};${c[1]};${c[2]}m`
const reset = "\x1b[0m"

function draw(line: string, color: RGB, shadow: RGB) {
  return [...line]
    .map((char) => {
      if (char === "_") return `${bg(shadow)} ${reset}`
      if (char === "^") return `${fg(color)}${bg(shadow)}▀${reset}`
      if (char === "~") return `${fg(shadow)}▀${reset}`
      if (char === " ") return " "
      return `${fg(color)}${char}${reset}`
    })
    .join("")
}

const plain = (line: string) => line.replace(/[_~]/g, " ").replace(/\^/g, "▀")

// The logo as terminal lines: truecolor blue, or plain glyphs when colour is off.
export function ansiLogo(pad = "", color = true) {
  const { left, right, leftShadow, rightShadow } = Brand.colors
  return glyphs.left.map((line, i) => {
    const r = glyphs.right[i] ?? ""
    if (!color) return `${pad}${plain(line)} ${plain(r)}`.trimEnd()
    return `${pad}${draw(line, left, leftShadow)} ${draw(r, right, rightShadow)}`
  })
}
