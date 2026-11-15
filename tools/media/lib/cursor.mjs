// What the film draws over the page, all of it into the page (a screenshot of an Electron page does not contain the mouse pointer, and a cut or a speed-up of the
// frames then can never put the picture out of step with what it says): the arrow that glides to its target, the ring that plays at a click, the badge of a key
// that is pressed, and the caption strip over the status bar (docs/18-release-0.2/04-gifs-v2.md §3, §6.1). The real mouse is moved too, so hover states appear.
// The page is built with DOM calls, not innerHTML: VS Code's workbench requires Trusted Types.
import { window as size } from "../config.mjs";

// the arrow's outline
const ARROW_PATH = "M2 1.5v19l4.9-4.6 3.3 7.6 3.1-1.4-3.2-7.4h6.8z";

export const MOVE_MS = 450;
export const RING_MS = 350;
/** how long the badge of a key stays (a film; the stills keep it until the next beat) */
export const KEY_MS = 800;

const MODIFIERS = { Meta: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧" };
const NAMED = { Enter: "⏎", Escape: "Esc", ArrowLeft: "←", ArrowRight: "→", ArrowUp: "↑", ArrowDown: "↓", Backspace: "⌫", Tab: "⇥", " ": "Space", Space: "Space" };

/** What a badge says for one key combination in Playwright's notation (`Meta+Shift+p` → `⌘⇧P`, `Enter` → `⏎`, `f` → `F`). */
export function keyLabel(combo) {
  return combo
    .split("+")
    .map((part) => MODIFIERS[part] ?? NAMED[part] ?? (part.length === 1 ? part.toUpperCase() : part))
    .join("");
}

export class Cursor {
  #page;
  #rec;
  #x = size.width / 2;
  #y = size.height / 2;
  #caption = "";

  constructor(page) {
    this.#page = page;
  }

  /** The recording that gets a mark for every glide and drag (`glide`, `drag`: what the edit speeds up, §7.2). */
  attach(recording) {
    this.#rec = recording;
  }

  /** Puts the arrow, the key badge and the caption strip in the page (again after a reload). */
  async install() {
    await this.#page.evaluate(
      ({ path, x, y, move, ring, key, caption }) => {
        for (const id of ["media-cursor", "media-ring", "media-key", "media-caption", "media-cursor-style"]) document.getElementById(id)?.remove();
        const style = document.createElement("style");
        style.id = "media-cursor-style";
        style.textContent = `
          #media-cursor { position: fixed; left: 0; top: 0; z-index: 2147483647; pointer-events: none; transform: translate(${x}px, ${y}px); transition: transform ${move}ms ease-in-out; }
          #media-ring { position: fixed; left: 0; top: 0; z-index: 2147483646; pointer-events: none; width: 34px; height: 34px; margin: -17px 0 0 -17px; border-radius: 50%;
            border: 3px solid #4db0ff; opacity: 0; }
          #media-ring.play { animation: media-ring ${ring}ms ease-out; }
          @keyframes media-ring { from { opacity: 0.95; transform: scale(0.3); } to { opacity: 0; transform: scale(1.25); } }
          #media-key { position: fixed; left: 50%; z-index: 2147483645; pointer-events: none; transform: translateX(-50%); display: none; padding: 7px 18px; border-radius: 9px;
            background: #0b0d12; color: #fff; border: 2px solid #4db0ff; font: 600 22px/1.2 -apple-system, "SF Pro Text", system-ui, sans-serif; letter-spacing: 1px; white-space: nowrap; }
          #media-key.on { display: block; }
          #media-key.timed { animation: media-key ${key}ms linear forwards; }
          @keyframes media-key { 0% { opacity: 1; } 88% { opacity: 1; } 100% { opacity: 0; } }
          #media-caption { position: fixed; left: 0; right: 0; z-index: 2147483645; pointer-events: none; display: none; align-items: center; justify-content: center;
            background: #0b0d12; color: #f2f2f2; border-top: 1px solid #3a3d41; font: 600 14px/1 -apple-system, "SF Pro Text", system-ui, sans-serif; white-space: nowrap; overflow: hidden; }
          #media-caption.on { display: flex; }
          .media-hidden { display: none !important; }`;
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
        const keyEl = document.createElement("div");
        keyEl.id = "media-key";
        const captionEl = document.createElement("div");
        captionEl.id = "media-caption";
        captionEl.textContent = caption;
        captionEl.classList.toggle("on", caption !== "");
        document.body.append(captionEl, keyEl, ringEl, el);
      },
      { path: ARROW_PATH, x: this.#x, y: this.#y, move: MOVE_MS, ring: RING_MS, key: KEY_MS, caption: this.#caption },
    );
    await this.#place();
  }

  /**
   * The caption strip covers the status bar (the one part of the window no story needs), and the key badge sits above it, at the bottom centre. Measured from
   * the real status bar, so a different window size or zoom needs nothing here.
   */
  async #place() {
    await this.#page.evaluate(() => {
      const bar = document.querySelector(".statusbar")?.getBoundingClientRect();
      const top = bar && bar.height > 0 ? bar.top : window.innerHeight - 22;
      const height = bar && bar.height > 0 ? bar.height : 22;
      const caption = document.getElementById("media-caption");
      caption.style.top = `${top}px`;
      caption.style.height = `${height}px`;
      document.getElementById("media-key").style.bottom = `${window.innerHeight - top + 14}px`;
    });
  }

  /**
   * Shows the badge of a key combination (`⏎`, `⌘S`, `⌘K ⌘M`) at the bottom centre for KEY_MS, then it goes by itself. `sticky`: it stays until `clearKey()` (the
   * stills, which are taken after the key has long gone).
   */
  async key(label, { sticky = false } = {}) {
    await this.#page.evaluate(({ label, sticky }) => {
      const el = document.getElementById("media-key");
      el.textContent = label;
      el.classList.remove("on", "timed");
      void el.offsetWidth; // restart the animation when a key follows a key
      el.classList.add("on");
      if (!sticky) el.classList.add("timed");
    }, { label, sticky });
  }

  async clearKey() {
    await this.#page.evaluate(() => document.getElementById("media-key")?.classList.remove("on", "timed"));
  }

  /** The caption (six words or fewer): drawn into the page, so no cut can leave it out of step. An empty text hides the strip. */
  async caption(text) {
    this.#caption = text;
    await this.#page.evaluate((text) => {
      const el = document.getElementById("media-caption");
      el.textContent = text;
      el.classList.toggle("on", text !== "");
    }, text);
    await this.#place();
  }

  get captionText() {
    return this.#caption;
  }

  /**
   * The box of `locator` once it has stopped moving (a camera gliding or a list settling would send the arrow to where the target was). A scene calls this
   * while it holds, for the target of the next action, and passes the box on (`{ box }`): measuring takes a quarter of a second, and doing it after the hold
   * is a freeze.
   */
  async aim(locator) {
    let last = await locator.boundingBox();
    for (let i = 0; i < 40; i++) {
      await this.#page.waitForTimeout(40);
      const box = await locator.boundingBox();
      if (box && last && Math.abs(box.x - last.x) < 1 && Math.abs(box.y - last.y) < 1) return box;
      last = box;
    }
    if (!last) throw new Error("the cursor has nothing to move to: " + locator);
    return last;
  }

  /**
   * Moves to the middle of `locator` (a point `at` = [fx, fy] of its box if given), then waits for the glide to end. `box`: the target, already measured by
   * `aim`. `hover: false`: no Playwright hover afterwards (a card on the canvas has no hover-only buttons; a hover costs a round trip, which is a freeze).
   */
  async moveTo(locator, at = [0.5, 0.5], { box, hover = true } = {}) {
    box ??= await this.aim(locator);
    await this.moveToPoint(box.x + box.width * at[0], box.y + box.height * at[1]);
    // the pointer's hover states (a row's buttons appear only on hover) are Playwright's, which reaches inside webviews
    if (hover) await locator.hover({ position: { x: box.width * at[0], y: box.height * at[1] }, timeout: 10_000 }).catch(() => {});
  }

  /** The arrow glides; the real pointer is not moved along the way (a straight line over a list would hover every row, and tooltips would pop up behind it). */
  async moveToPoint(x, y) {
    const end = this.#rec?.begin("glide");
    this.#x = x;
    this.#y = y;
    await this.#page.evaluate(({ x, y }) => (document.getElementById("media-cursor").style.transform = `translate(${x}px, ${y}px)`), { x, y });
    await this.#page.waitForTimeout(MOVE_MS + 30);
    end?.();
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

  /** The ring plays and the click lands where the arrow is. With a measured `box` the click is the pointer's own (fast); without, Playwright's, which checks what it hits. */
  async click(locator, { at = [0.5, 0.5], button = "left", box, hover } = {}) {
    await this.moveTo(locator, at, { box, hover: hover ?? !box });
    await this.#ring();
    if (box) {
      await this.#page.mouse.click(this.#x, this.#y, { button });
    } else {
      const b = await locator.boundingBox();
      await locator.click({ position: { x: b.width * at[0], y: b.height * at[1] }, button, timeout: 15_000 });
    }
    await this.#page.waitForTimeout(RING_MS);
  }

  /** A click on a point of the page (empty canvas, which no locator names): the arrow glides there, the ring plays, the pointer clicks. */
  async clickPoint(x, y) {
    await this.moveToPoint(x, y);
    await this.#ring();
    await this.#page.mouse.click(x, y);
    await this.#page.waitForTimeout(RING_MS);
  }

  async dblclick(locator, { at = [0.5, 0.5], box, hover } = {}) {
    await this.moveTo(locator, at, { box, hover: hover ?? !box });
    await this.#ring();
    if (box) {
      await this.#page.mouse.dblclick(this.#x, this.#y);
    } else {
      const b = await locator.boundingBox();
      await locator.dblclick({ position: { x: b.width * at[0], y: b.height * at[1] }, timeout: 15_000 });
    }
    await this.#page.waitForTimeout(RING_MS);
  }

  /**
   * A drag, as a hand does it: the arrow glides to `from`, presses, and goes to `to` over `ms` while the real pointer moves in small steps, then releases.
   * `from` and `to` are page coordinates; `from` must be on something that pans (the canvas background).
   */
  async drag([fx, fy], [tx, ty], ms = 700) {
    await this.moveToPoint(fx, fy);
    const end = this.#rec?.begin("drag");
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
    end?.();
  }

  /** The arrow and its ring hidden (G2 is keys only: no mouse is drawn); the key badge and the caption stay. */
  async hideArrow(hidden = true) {
    await this.#page.evaluate((h) => {
      for (const id of ["media-cursor", "media-ring"]) document.getElementById(id)?.classList.toggle("media-hidden", h);
    }, hidden);
  }

  /** A screenshot of a picture (not a film, not a still of the storyboard) must not show the arrow, the badge or the caption (except where the scene says so). */
  async hide(hidden = true) {
    await this.#page.evaluate((h) => {
      for (const id of ["media-cursor", "media-ring", "media-key", "media-caption"]) document.getElementById(id)?.classList.toggle("media-hidden", h);
    }, hidden);
  }
}
