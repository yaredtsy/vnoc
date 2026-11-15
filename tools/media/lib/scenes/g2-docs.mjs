// G2 docs.gif: "Write a doc. Rename the function. The doc stays." (docs/18-release-0.2/04-gifs-v2.md §5). Two panes that never move, two acts with a pause between them, and one
// mouse gesture: the right-click that starts the first act (the same gesture as G1's Show Calls); the arrow then leaves and is hidden, and every other step is a key with its
// badge. The left pane is where the hands are (`_client.py`, then the doc as a tab over it, then `_client.py` again); the right pane, the docs tab, only answers.
//
// Setup, off camera: the window 960 × 600, the Activity Bar and the side bar closed, `httpx/_client.py` on the left with `def _send_handling_redirects(` a third of the way
// down and the text cursor in the name, and on the right the docs tab (Show Docs from the editor's menu: unpinned, it shows the cursor's node), which says the function has no
// doc, and the arrow at rest right of the signature, on no text. `_send_handling_redirects` has none in the overlay (01 §2.2).
//
// The scene records freely (§6.1): it waits for state, holds 1.5 s after every change, and never thinks about length. Every row of the table of §5 is a beat.
import { nameInLine, scrollNameTo } from "../editor.mjs";
import { settled } from "../canvas.mjs";
import { recordingFor } from "../stills.mjs";
import { webviewWith } from "../vscode.mjs";

export const id = "g2";
export const kind = "gif";
export const output = "docs.gif";
export const stills = true;
// Nothing in the code is highlighted or offered: no other occurrence of a word, no bracket match, no suggestion list while the name is typed, no hover. The Activity Bar is closed.
export const settings = {
  "workbench.activityBar.location": "hidden",
  "editor.occurrencesHighlight": "off",
  "editor.selectionHighlight": false,
  "editor.matchBrackets": "never",
  "editor.quickSuggestions": { other: false, comments: false, strings: false },
  "editor.suggestOnTriggerCharacters": false,
  "editor.wordBasedSuggestions": "off",
  "editor.parameterHints.enabled": false,
  "editor.hover.enabled": false,
};

const HOLD = 1500; // after every change (§6.1); the edit trims it
const END = 3000;
const TYPING_MS = 15; // 04 §5 beat 4: typed at 15 ms a key
const FN = "_send_handling_redirects";
const RENAMED = "_follow_redirects";
const TEXT = "Follows redirects, one request per hop.\n\n- stops at `max_redirects`\n- every hop is added to `history`";

const left = (page) => page.locator(".editor-group-container").first();
const quickInput = (page) => page.locator(".quick-input-widget");

/** Everything before the first frame. Returns the docs tab's frame and the name of the function in the code. */
async function setup(stage) {
  const { page, cursor } = stage;
  await stage.openRow("folder httpx,");
  await stage.openRow("file _client.py,");
  await stage.openRow("class Client,");
  await stage.row(`function ${FN},`).click(); // the canvas is closed: the code, at the function
  await left(page).locator(".view-line", { hasText: `def ${FN}(` }).first().waitFor({ timeout: 15_000 });
  await stage.command("View: Toggle Primary Side Bar Visibility");
  await page.locator(".part.sidebar").waitFor({ state: "hidden", timeout: 5000 });
  await page.mouse.move(200, 20);

  const name = nameInLine(page, `def ${FN}(`, FN);
  await scrollNameTo(stage, name, 200);

  // A click on the name is the pointer's own, at a box that has stopped moving. Playwright's `locator.click` checks what it hits and, when that fails (the editor group
  // narrows while the docs tab opens), retries with the element scrolled to the start of its scroller: the code ended up scrolled up and to the left, in the first frame
  // of one take and from the right-click on in another. Nothing is scrolled.
  const clickName = async (button) => {
    const box = await cursor.aim(name);
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2, { button });
  };

  // the docs tab beside the code, from the editor's own menu: unpinned, it follows the cursor, and the cursor stays in the code
  const placed = await cursor.aim(name);
  await clickName("right");
  // each entry is hovered before it is clicked: a click that lands while the submenu is still opening is lost
  for (const entry of ["V-NOC", "Show Docs"]) {
    const item = page.getByRole("menuitem", { name: entry });
    await item.hover();
    await page.waitForTimeout(400);
    await item.click();
  }
  const docs = await webviewWith(page, "text=/No docs on/");
  // the text cursor in the name, and the keyboard in the code
  await clickName("left");
  await page.mouse.move(200, 20);
  const box = await cursor.aim(name);
  // the code is where the scene put it, a third of the way down: nothing has scrolled it since (the editor group is narrower now, so only the height is compared)
  if (Math.abs(box.y - placed.y) > 3) throw new Error(`the code moved during setup: the name was ${Math.round(placed.y)} px from the top and is ${Math.round(box.y)}`);
  // the arrow at rest right of the signature, on no text (the line ends a few characters after the name)
  await cursor.moveToPoint(box.x + box.width + 60, box.y + box.height / 2);
  await stage.caption("A function. No doc yet.");
  return { docs, name };
}

export async function run(stage, { stills: onlyStills }) {
  const { page, cursor } = stage;
  const { docs, name } = await setup(stage);
  const rec = recordingFor(stage, id, { stills: onlyStills });
  const idLine = left(page).locator(".view-line", { hasText: /ID:\s*[0-9A-Z]{26}/ }).first();
  const tab = (name) => page.locator(".tab .label-name", { hasText: name });

  await rec.start(); // returns when the capture loop is warm
  try {
    // 1 · a function, and a docs tab that says it has none
    await rec.beat("start");
    await rec.hold(HOLD, "first");

    // 2 · act 1, add: right-click the name → V-NOC ▸ Add Doc…, as G1 does Show Calls. The right-click puts the text cursor in the name, so the editor's menu is about this
    // function: the title box says so. Then the arrow leaves the code, to empty space in the docs tab, and is hidden for the rest of the film
    await rec.beat("add-doc");
    await stage.caption("Right-click → Add Doc…");
    // the right-click is the pointer's own, at a measured box: Playwright's `locator.click` retries a click whose hit-target check fails by scrolling the element to the
    // start of its scroller, and in a film that scrolled the code (up and to the left) for the rest of the film. Nothing is scrolled
    const where = await cursor.aim(name);
    await cursor.click(name, { button: "right", box: where });
    await cursor.click(page.getByRole("menuitem", { name: "V-NOC" }));
    await cursor.click(page.getByRole("menuitem", { name: /^Add Doc/ }));
    const now = await name.boundingBox();
    if (!now || Math.abs(now.x - where.x) > 2 || Math.abs(now.y - where.y) > 2) throw new Error(`the code moved while the menu was used: the name was at ${Math.round(where.x)}, ${Math.round(where.y)} and is at ${now ? `${Math.round(now.x)}, ${Math.round(now.y)}` : "nowhere"}`);
    await rec.during("reveal", "title box", () => quickInput(page).locator(".quick-input-title", { hasText: /Add doc/i }).waitFor({ timeout: 15_000 }));
    const message = ((await quickInput(page).locator(".quick-input-message").textContent().catch(() => "")) ?? "").trim();
    const value = await quickInput(page).locator("input.input").inputValue();
    rec.note(`the title box: "${message}", filled with "${value}"`);
    console.log(`  the title box: "${message}", filled with "${value}"`);
    if (!message.includes(FN)) throw new Error(`Add Doc… from the editor's menu is not about ${FN}: the title box says "${message}"`);
    const away = [720, 480]; // empty space in the docs tab, away from its button
    await page.mouse.move(away[0], away[1]);
    await cursor.moveToPoint(away[0], away[1]);
    await cursor.hideArrow();
    await rec.hold(HOLD);

    // 3 · Enter: the title stays Overview, the core writes the ID line, the doc opens as a tab over the code
    await rec.beat("title");
    await stage.caption("Give it a title");
    await stage.key("Enter");
    await rec.during("reveal", "overview.md", () => tab("overview.md").waitFor({ timeout: 30_000 }));
    await rec.during("wait", "Add Doc", () => settled(page, null, 800));
    await rec.hold(HOLD);

    // 4 · the doc's text, typed at 15 ms a key
    await rec.beat("write");
    await stage.caption("Write it in Markdown");
    await rec.during("typing", "the doc's text", () => page.keyboard.type(TEXT, { delay: TYPING_MS }));
    await rec.hold(HOLD);

    // 5 · ⌘S, then ⌘W: the code is back with the ID line under the def, and the docs tab shows the new doc, rendered
    await rec.beat("save");
    await stage.caption("Saved: one ID line links them");
    await rec.during("reveal", "saved", async () => {
      await stage.key("Meta+s");
      await page.locator(".tab.dirty").waitFor({ state: "detached", timeout: 15_000 });
    });
    await stage.key("Meta+w");
    await rec.during("reveal", "the doc appears", async () => {
      await idLine.waitFor({ timeout: 15_000 });
      await docs.getByText("Follows redirects").first().waitFor({ timeout: 30_000 });
    });
    await rec.during("wait", "index", () => settled(page, null, 500));
    await rec.hold(HOLD);

    // 6 · the pause: nothing moves
    await rec.beat("pause");
    await rec.hold(HOLD, "read");

    // 7 · act 2, rename: ⌘D selects the name (the text cursor is in it), the new name is typed over it
    await rec.beat("rename");
    await stage.caption("Now rename the function");
    await stage.key("Meta+d");
    await page.locator(".statusbar", { hasText: new RegExp(`\\(${FN.length} selected\\)`) }).waitFor({ timeout: 5000 });
    await rec.hold(500, "after");
    await rec.during("typing", "the new name", () => page.keyboard.type(RENAMED, { delay: TYPING_MS * 2 }));
    await rec.hold(HOLD);

    // 8 · ⌘S: the docs tab's header becomes the new name and the doc is still there
    await rec.beat("end");
    await stage.caption("The doc is still attached");
    await stage.key("Meta+s");
    // the project re-indexes and the docs tab says "Put the cursor in a function or class…" for about half a second: that is time in which nothing the viewer should see is
    // happening, a wait, from ⌘S until the header says the new name. The frame where it does, and the doc still under it, is the reveal
    await rec.during("wait", "re-index", () => docs.getByText(RENAMED).first().waitFor({ timeout: 30_000 }));
    await rec.during("reveal", "renamed", async () => {
      await docs.getByText("Follows redirects").first().waitFor({ timeout: 5000 });
      await settled(page, null, 300);
    });
    await rec.hold(END, "end");
  } catch (error) {
    await rec.stop().catch(() => {});
    throw error;
  }
  return { rec, stats: await rec.stop() };
}
