// What a scene needs to know about the code editor, and nothing about a particular scene.

/**
 * Scrolls the editor (setup only: a film never scrolls) until `name`, the span of a word in the code, is `wanted` px from the window's top: the reveal puts a function
 * in the middle of the pane, and the first frame of a film wants it a third of the way down. A wheel step moves less than it is told, so it is repeated, measuring.
 * Returns the box of the name.
 */
export async function scrollNameTo(stage, name, wanted = 200) {
  const { page, cursor } = stage;
  let box = await cursor.aim(name);
  for (let round = 0; round < 6 && Math.abs(box.y - wanted) > 15; round++) {
    await page.mouse.move(box.x + 300, box.y);
    await page.mouse.wheel(0, box.y - wanted);
    await page.mouse.move(200, 20);
    box = await cursor.aim(name);
  }
  if (Math.abs(box.y - wanted) > 40) throw new Error(`the name is ${Math.round(box.y)} px from the top, not about ${wanted}`);
  return box;
}

/** The span of a word in the line of code that has `lineText` (`def get(`): found by what is on the screen, not by a class of the editor's own making. */
export const nameInLine = (page, lineText, word) => page.locator(".view-line", { hasText: lineText }).locator("span", { hasText: new RegExp(`^${word}$`) }).first();
