// G2 docs.gif: "Write a doc. Rename the function. The doc stays." (docs/18-release-0.2/04-gifs-v2.md §5). Two panes that never move, two acts with a pause between them,
// and no mouse at all: the arrow is hidden and every step is a key with its badge. The left pane is where the hands are (`_client.py`, then the doc as a tab over it, then
// `_client.py` again); the right pane, the docs tab, only answers.
//
// Setup, off camera: the window 960 × 600, the Activity Bar and the side bar closed, `httpx/_client.py` on the left with `def _send_handling_redirects(` a third of the way
// down and the text cursor in the name, and on the right the docs tab (Show Docs from the editor's menu: unpinned, it shows the cursor's node), which says the function has no
// doc. `_send_handling_redirects` has none in the overlay (01 §2.2). Setup uses the mouse; the film does not.
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
const palette = (page) => page.locator(".quick-input-widget");

/** Everything before the first frame. Returns the docs tab's frame. */
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

  // the docs tab beside the code, from the editor's own menu: unpinned, it follows the cursor, and the cursor stays in the code
  await name.click({ button: "right" });
  // each entry is hovered before it is clicked: a click that lands while the submenu is still opening is lost
  for (const entry of ["V-NOC", "Show Docs"]) {
    const item = page.getByRole("menuitem", { name: entry });
    await item.hover();
    await page.waitForTimeout(400);
    await item.click();
  }
  const docs = await webviewWith(page, "text=/No docs on/");
  // the text cursor in the name, and the keyboard in the code
  await name.click();
  await page.mouse.move(200, 20);
  await cursor.hideArrow(); // keys only
  await stage.caption("A function. No doc yet.");
  return { docs };
}

export async function run(stage, { stills: onlyStills }) {
  const { page } = stage;
  const { docs } = await setup(stage);
  const rec = recordingFor(stage, id, { stills: onlyStills });
  const idLine = left(page).locator(".view-line", { hasText: /ID:\s*[0-9A-Z]{26}/ }).first();
  const tab = (name) => page.locator(".tab .label-name", { hasText: name });

  await rec.start(); // returns when the capture loop is warm
  try {
    // 1 · a function, and a docs tab that says it has none
    await rec.beat("start");
    await rec.hold(HOLD, "first");

    // 2 · act 1, add: F1, `add doc`, Enter. The palette's first entry is Add Doc…, and it takes the function at the cursor: the title box says so
    await rec.beat("add-doc");
    await stage.caption("Add Doc…");
    await stage.key("F1");
    await palette(page).locator("input.input").waitFor({ state: "visible", timeout: 5000 });
    await page.keyboard.type("add doc", { delay: 60 });
    const first = palette(page).locator(".quick-input-list .monaco-list-row").first();
    await first.waitFor({ state: "visible", timeout: 5000 });
    for (let i = 0; i < 40 && !(((await first.getAttribute("aria-label")) ?? "").includes("Add Doc")); i++) await page.waitForTimeout(100);
    const entry = ((await first.getAttribute("aria-label")) ?? "").trim();
    if (!entry.includes("Add Doc")) throw new Error(`the palette's first entry for "add doc" is "${entry}", not V-NOC: Add Doc…`);
    rec.note(`the palette's first entry: "${entry}"`);
    console.log(`  the palette's first entry: "${entry}"`);
    await rec.look("the palette");
    await rec.hold(HOLD);
    await rec.during("reveal", "title box", async () => {
      await stage.key("Enter");
      await palette(page).locator(".quick-input-title", { hasText: /Add doc/i }).waitFor({ timeout: 15_000 });
    });
    const message = ((await palette(page).locator(".quick-input-message").textContent().catch(() => "")) ?? "").trim();
    const value = await palette(page).locator("input.input").inputValue();
    rec.note(`the title box: "${message}", filled with "${value}"`);
    console.log(`  the title box: "${message}", filled with "${value}"`);
    if (!message.includes(FN)) throw new Error(`Add Doc… from the palette is not about ${FN}: the title box says "${message}"`);
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
    await rec.during("reveal", "renamed", async () => {
      await stage.key("Meta+s");
      await docs.getByText(RENAMED).first().waitFor({ timeout: 30_000 });
      await docs.getByText("Follows redirects").first().waitFor({ timeout: 5000 });
    });
    await rec.during("wait", "index", () => settled(page, null, 500));
    await rec.hold(END, "end");
  } catch (error) {
    await rec.stop().catch(() => {});
    throw error;
  }
  return { rec, stats: await rec.stop() };
}
