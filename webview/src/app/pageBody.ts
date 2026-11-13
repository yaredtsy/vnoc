// What the page body shows (docs/16-ui-redesign/02-top-bar.md §4). Pure, so the six cases are tested; App only draws the answer.

export type Status = "loading" | "ready" | "error";
export type Body = "canvas" | "loading" | "error";

/**
 * Cards drawn stay, whatever a later read says (the bar's ⚠ tells): the canvas. With nothing drawn yet the page says the first read is on its way,
 * or why it failed. A message is never beside the canvas.
 */
export function pageBody(status: Status, hasLayout: boolean): Body {
  if (hasLayout) return "canvas";
  return status === "error" ? "error" : "loading";
}
