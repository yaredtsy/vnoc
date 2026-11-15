// A screenshot of an Electron page does not contain the mouse pointer, so the pointer is drawn into the page: an arrow that glides to its target, and a ring
// that plays at a click (docs/18-release-0.2/01-media.md §4.3). The real mouse is moved too, so hover states appear.
// the arrow's outline; the page is built with DOM calls, not innerHTML: VS Code's workbench requires Trusted Types
const ARROW_PATH = "M2 1.5v19l4.9-4.6 3.3 7.6 3.1-1.4-3.2-7.4h6.8z";

export const MOVE_MS = 330;
export const RING_MS = 180;

export class Cursor {
  #page;
  #x = 640;
  #y = 400;

  constructor(page) {
    this.#page = page;
  }

  /** Puts the arrow in the page (again after a reload). */
  async install() {
    await this.#page.evaluate(
      ({ path, x, y, move, ring }) => {
        document.getElementById("media-cursor")?.remove();
        const style = document.createElement("style");
        style.id = "media-cursor-style";
        style.textContent = `
          #media-cursor { position: fixed; left: 0; top: 0; z-index: 2147483647; pointer-events: none; transform: translate(${x}px, ${y}px); transition: transform ${move}ms ease-in-out; }
          #media-cursor.hidden { display: none; }
          #media-ring { position: fixed; left: 0; top: 0; z-index: 2147483646; pointer-events: none; width: 34px; height: 34px; margin: -17px 0 0 -17px; border-radius: 50%;
            border: 3px solid #4db0ff; opacity: 0; }
          #media-ring.play { animation: media-ring ${ring}ms ease-out; }
          @keyframes media-ring { from { opacity: 0.95; transform: scale(0.3); } to { opacity: 0; transform: scale(1.25); } }`;
        document.getElementById("media-cursor-style")?.remove();
        document.head.append(style);
        const el = document.createElement("div");
        el.id = "media-cursor";
        const ns = "http://www.w3.org/2000/svg";
        const svg = document.createElementNS(ns, "svg");
        for (const [k, v] of Object.entries({ width: "22", height: "26", viewBox: "0 0 22 26" })) svg.setAttribute(k, v);
        const outline = document.createElementNS(ns, "path");
        for (const [k, v] of Object.entries({ d: path, fill: "#fff", stroke: "#111", "stroke-width": "1.6", "stroke-linejoin": "round" })) outline.setAttribute(k, v);
        svg.append(outline);
        el.append(svg);
        const ringEl = document.createElement("div");
        ringEl.id = "media-ring";
        document.body.append(ringEl, el);
      },
      { path: ARROW_PATH, x: this.#x, y: this.#y, move: MOVE_MS, ring: RING_MS },
    );
  }

  /** The box of `locator` once it has stopped moving (a camera gliding or a list settling would send the arrow to where the target was). */
  async #stableBox(locator) {
    let last = await locator.boundingBox();
    for (let i = 0; i < 40; i++) {
      await this.#page.waitForTimeout(60);
      const box = await locator.boundingBox();
      if (box && last && Math.abs(box.x - last.x) < 1 && Math.abs(box.y - last.y) < 1) return box;
      last = box;
    }
    if (!last) throw new Error("the cursor has nothing to move to: " + locator);
    return last;
  }

  /** Moves to the middle of `locator` (a point `at` = [fx, fy] of its box if given), then waits for the glide to end. */
  async moveTo(locator, at = [0.5, 0.5]) {
    const box = await this.#stableBox(locator);
    await this.moveToPoint(box.x + box.width * at[0], box.y + box.height * at[1]);
    // the pointer's hover states (a row's buttons appear only on hover) are Playwright's, which reaches inside webviews
    await locator.hover({ position: { x: box.width * at[0], y: box.height * at[1] }, timeout: 3000 }).catch(() => {});
  }

  /** The arrow glides; the real pointer is not moved along the way (a straight line over a list would hover every row, and tooltips would pop up behind it). */
  async moveToPoint(x, y) {
    this.#x = x;
    this.#y = y;
    await this.#page.evaluate(({ x, y }) => (document.getElementById("media-cursor").style.transform = `translate(${x}px, ${y}px)`), { x, y });
    await this.#page.waitForTimeout(MOVE_MS + 30);
  }

  async #ring() {
    await this.#page.evaluate(({ x, y }) => {
      const ring = document.getElementById("media-ring");
      ring.style.transform = "";
      ring.style.left = `${x}px`;
      ring.style.top = `${y}px`;
      ring.classList.remove("play");
      void ring.offsetWidth;
      ring.classList.add("play");
    }, { x: this.#x, y: this.#y });
  }

  async click(locator, options = {}) {
    const at = options.at ?? [0.5, 0.5];
    await this.moveTo(locator, options.at);
    await this.#ring();
    const box = await locator.boundingBox();
    await locator.click({ position: { x: box.width * at[0], y: box.height * at[1] }, button: options.button ?? "left", timeout: 5000 });
    await this.#page.waitForTimeout(RING_MS);
  }

  async dblclick(locator, options = {}) {
    const at = options.at ?? [0.5, 0.5];
    await this.moveTo(locator, options.at);
    await this.#ring();
    const box = await locator.boundingBox();
    await locator.dblclick({ position: { x: box.width * at[0], y: box.height * at[1] }, timeout: 5000 });
    await this.#page.waitForTimeout(RING_MS);
  }

  /**
   * A drag, as a hand does it: the arrow glides to `from`, presses, and goes to `to` over `ms` while the real pointer moves in small steps, then releases.
   * `from` and `to` are page coordinates; `from` must be on something that pans (the canvas background).
   */
  async drag([fx, fy], [tx, ty], ms = 700) {
    await this.moveToPoint(fx, fy);
    await this.#page.mouse.move(fx, fy);
    await this.#ring();
    await this.#page.mouse.down();
    const steps = 14;
    await this.#page.evaluate(({ x, y, ms }) => {
      const el = document.getElementById("media-cursor");
      el.style.transitionDuration = `${ms}ms`;
      el.style.transform = `translate(${x}px, ${y}px)`;
    }, { x: tx, y: ty, ms });
    for (let i = 1; i <= steps; i++) {
      await this.#page.mouse.move(fx + ((tx - fx) * i) / steps, fy + ((ty - fy) * i) / steps);
      await this.#page.waitForTimeout(ms / steps);
    }
    await this.#page.mouse.up();
    await this.#page.evaluate((move) => (document.getElementById("media-cursor").style.transitionDuration = `${move}ms`), MOVE_MS);
    this.#x = tx;
    this.#y = ty;
    await this.#page.waitForTimeout(150);
  }

  /** A screenshot must not show the arrow (except where the scene says so). */
  async hide(hidden = true) {
    await this.#page.evaluate((h) => document.getElementById("media-cursor")?.classList.toggle("hidden", h), hidden);
  }
}
