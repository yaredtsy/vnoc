// How wide one character of the code font is, in px, as CodeEditor draws it (11 px, the editor font). Measured on a canvas: the layout needs it before the card's Monaco exists.

export const CODE_FONT_SIZE = 11;

const known = new Map<string, number>();

export function codeCharWidth(): number {
  const family = getComputedStyle(document.body).getPropertyValue("--editor-font") || "monospace";
  const font = `${CODE_FONT_SIZE}px ${family}`;
  let w = known.get(font);
  if (w === undefined) {
    const ctx = document.createElement("canvas").getContext("2d")!;
    ctx.font = font;
    w = ctx.measureText("0".repeat(100)).width / 100;
    if (document.fonts.check(font)) known.set(font, w); // a font still loading measures as its fallback: ask again next time
  }
  return w;
}
