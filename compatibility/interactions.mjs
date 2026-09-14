// Shared native interactions; each suite still predicts and checks its own scene.
export function waitForSelection(page, ids) {
  return page.waitForFunction(ids => {
    const selected = Object.keys(window.editor.getAppState().selectedElementIds)
      .filter(id => window.editor.getAppState().selectedElementIds[id]);
    return selected.length === ids.length && ids.every(id => selected.includes(id));
  }, ids);
}

export async function recolorSelection(page, ids, color) {
  await page.getByRole("button", { name: "Stroke", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Stroke", exact: true });
  await input.fill(color.slice(1));
  await input.press("Enter");
  await page.waitForFunction(({ ids, color }) => window.editor.getSceneElements()
    .filter(e => ids.includes(e.id)).every(e => e.strokeColor === color), { ids, color });
  await page.mouse.click(1050, 600);
  await page.waitForFunction(() => !document.querySelector('input[aria-label="Stroke"]'));
}
