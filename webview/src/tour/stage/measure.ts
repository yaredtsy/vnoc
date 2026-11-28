import type { StageLayout } from "@vnoc/shared/messages.ts";

/**
 * How the stage is laid out, read from the page after it drew (`stageDrawn.layout`, for the extension's suite: a narrow canvas must draw the stage stacked, with nothing cut and
 * `Next ›` reachable). Reads the DOM and changes nothing.
 */
export function measureStage(stage: HTMLElement, narrow: boolean, folded: boolean): StageLayout {
  const box = stage.getBoundingClientRect();
  const inside = (el: Element): boolean => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.left >= box.left - 1 && r.right <= box.right + 1 && r.top >= box.top - 1 && r.bottom <= box.bottom + 1;
  };
  const named = (el: Element): string => el.getAttribute("aria-label") ?? el.textContent?.trim() ?? el.className;
  // the header's buttons and the foot's: the ones the reader needs. A button that is `display: none` has no box and is not asked for
  const needed = Array.from(stage.querySelectorAll("button.stage-icon, .stage-foot .stage-btn")).filter((el) => el.getBoundingClientRect().width > 0);
  const code = stage.querySelector<HTMLElement>('[data-part="code"]');
  const lines = Array.from(stage.querySelectorAll<HTMLElement>(".stage-line"));
  const body = stage.querySelector<HTMLElement>(".stage-body");
  let codeRows = 0;
  if (code) {
    const top = code.getBoundingClientRect();
    codeRows = lines.filter((l) => {
      const r = l.getBoundingClientRect();
      return r.top >= top.top - 1 && r.bottom <= top.bottom + 1;
    }).length;
  }
  // the chip is under the code, never over a row of it: a row counts as covered where it is in view (inside the code pane's box) and the chip's box meets it
  const chip = stage.querySelector<HTMLElement>(".stage-more");
  const chipOverCode = (() => {
    if (!chip || !code) return null;
    const c = chip.getBoundingClientRect();
    const view = code.getBoundingClientRect();
    return lines.some((l) => {
      const r = l.getBoundingClientRect();
      const top = Math.max(r.top, view.top);
      const bottom = Math.min(r.bottom, view.bottom);
      return bottom > top && top < c.bottom - 0.5 && bottom > c.top + 0.5 && r.left < c.right && r.right > c.left;
    });
  })();
  return {
    narrow,
    folded,
    stacked: body !== null && getComputedStyle(body).flexDirection === "column",
    canvasHeight: stage.parentElement?.clientHeight ?? 0,
    width: Math.round(box.width),
    height: Math.round(box.height),
    codeHeight: code ? Math.round(code.clientHeight) : 0,
    codeRows,
    wrapped: lines.filter((l) => l.getBoundingClientRect().height > 20).length,
    codeCut: code ? code.scrollWidth > code.clientWidth + 1 : false,
    outside: needed.filter((el) => !inside(el)).map(named),
    chipOverCode,
    coverParts: Array.from(stage.querySelectorAll('[data-part="cover"] > [data-part]')).map((el) => el.getAttribute("data-part")!),
  };
}
