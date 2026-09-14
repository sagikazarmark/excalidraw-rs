import assert from "node:assert/strict";
import { readFile,writeFile } from "node:fs/promises";
import path from "node:path";

export async function verifyGroups(page,results) {
  const folder=process.env.SERIES_DIR || path.join(import.meta.dirname,"../output/two-series");
  const documents={};
  for (const name of ["clean","sketch-1","sketch-2"]) {
    const json=await readFile(path.join(folder,`${name}.excalidraw`),"utf8");
    const raw=JSON.parse(json).elements;
    documents[name]=json;
    const imported=await page.evaluate(json=>window.checks.load(json),json);
    assert.equal(imported.length,34);
    for (const [i,e] of imported.entries()) {
      for (const field of ["id","groupIds","seed","roughness","fillStyle","strokeColor","opacity"]) assert.deepEqual(e[field],raw[i][field]);
    }
    await page.waitForFunction(id=>window.editor.getSceneElements()[0]?.id===id,raw[0].id);
    await page.screenshot({path:path.join(results,`${name}.png`)});
    await writeFile(path.join(results,`${name}.raw.excalidraw`),json);
  }
  const paintsJson=await readFile(path.join(folder,"paints.excalidraw"),"utf8");
  const paintsRaw=JSON.parse(paintsJson).elements;
  const paints=await page.evaluate(json=>window.checks.load(json),paintsJson);
  assert.deepEqual(paints.map(e=>[e.type,e.fillStyle,e.opacity,e.backgroundColor,e.strokeColor]),paintsRaw.map(e=>[e.type,e.fillStyle,e.opacity,e.backgroundColor,e.strokeColor]));
  assert.deepEqual(paints.slice(1,4).map(e=>[e.fillStyle,e.opacity]),[["solid",50],["hachure",50],["cross-hatch",50]]);
  assert.equal(paints[4].backgroundColor,"#ff0000");
  await page.screenshot({path:path.join(results,"paints.png")});
  await writeFile(path.join(results,"paints.raw.excalidraw"),paintsJson);
  const raw=JSON.parse(documents.clean).elements;
  await page.evaluate(json=>window.checks.load(json),documents.clean);
  const measured=raw.find(e=>e.text==="Measured");
  const group=measured.groupIds[0];
  const members=raw.filter(e=>e.groupIds.includes(group));
  assert.equal(members.length,3);
  const selected=()=>page.evaluate(()=>Object.keys(window.editor.getAppState().selectedElementIds).filter(id=>window.editor.getAppState().selectedElementIds[id]));
  // First click selects the entire chart; arrows move every element together.
  await page.mouse.click(100+measured.x+measured.width/2,100+measured.y+measured.height/2);
  await page.waitForFunction(n=>Object.keys(window.editor.getAppState().selectedElementIds).length===n,raw.length);
  await page.keyboard.press("ArrowRight");
  const moved=await page.evaluate(()=>window.editor.getSceneElements());
  assert.ok(moved.every((e,i)=>e.x===raw[i].x+1 && e.y===raw[i].y));
  await page.keyboard.press("ArrowLeft");
  // Ungroup the chart shell, leaving each series (path + legend) grouped.
  await page.keyboard.press("Control+Shift+G");
  await page.waitForFunction(outer=>window.editor.getSceneElements().every(e=>!e.groupIds.includes(outer)),measured.groupIds[1]);
  await page.keyboard.press("Escape");
  await page.mouse.click(100+measured.x+measured.width/2,100+measured.y+measured.height/2);
  await page.waitForFunction(()=>Object.keys(window.editor.getAppState().selectedElementIds).length===3);
  assert.deepEqual((await selected()).sort(),members.map(e=>e.id).sort());
  // Recolor through the native stroke picker; all three series members change.
  await page.getByRole("button",{name:"Stroke",exact:true}).click();
  await page.getByRole("textbox",{name:"Stroke",exact:true}).fill("2f9e44");
  await page.getByRole("textbox",{name:"Stroke",exact:true}).press("Enter");
  await page.waitForFunction(ids=>window.editor.getSceneElements().filter(e=>ids.includes(e.id)).every(e=>e.strokeColor==="#2f9e44"),members.map(e=>e.id));
  // Close the picker by clicking away; Escape cancels its color transaction.
  await page.mouse.click(950,600);
  await page.waitForFunction(()=>!document.querySelector('input[aria-label="Stroke"]'));
  const recolored=await page.evaluate(()=>window.editor.getSceneElements());
  assert.equal(recolored.filter(e=>members.some(m=>m.id===e.id) && e.strokeColor==="#2f9e44").length,3);
  assert.ok(recolored.filter(e=>!members.some(m=>m.id===e.id)).every(e=>e.strokeColor===raw.find(r=>r.id===e.id).strokeColor));
  // Regroup whole chart through the UI, then save/reopen the edited artifact.
  await page.mouse.click(120,120);
  await page.keyboard.press("Control+a");
  await page.keyboard.press("Control+g");
  await page.waitForFunction(()=>new Set(window.editor.getSceneElements().map(e=>e.groupIds.at(-1))).size===1);
  const edited=await page.evaluate(()=>window.checks.save());
  assert.equal(JSON.parse(edited).elements.filter(e=>e.strokeColor==="#2f9e44").length,3);
  await writeFile(path.join(results,"two-series.edited.excalidraw"),edited);
  await page.evaluate(json=>window.checks.load(json),edited);
  await page.screenshot({path:path.join(results,"two-series.edited.png")});

  // Copy actual editor selections and paste via the browser clipboard into a
  // second editor page. No concatenation of generated JSON element arrays.
  const destination=await page.context().newPage();
  await destination.goto("http://127.0.0.1:5173");
  await destination.waitForFunction(()=>window.editor && window.checks);
  await destination.evaluate(()=>window.editor.updateScene({appState:{viewBackgroundColor:"#d3f9d8",scrollX:20,scrollY:100,zoom:{value:0.5}}}));
  await page.context().grantPermissions(["clipboard-read","clipboard-write"]);
  for (const [index,json] of [documents.clean,documents["sketch-1"],documents.clean,paintsJson].entries()) {
    await page.bringToFront();
    await page.evaluate(json=>window.checks.load(json),json);
    await page.mouse.click(120,120);
    await page.keyboard.press("Control+a");
    await page.keyboard.press("Control+c");
    await page.waitForFunction(async expectedId=>{
      try { return JSON.parse(await navigator.clipboard.readText()).elements[0].id===expectedId; }
      catch { return false; }
    },JSON.parse(json).elements[0].id);
    await destination.bringToFront();
    const x=250+(index%2)*470;
    const y=250+Math.floor(index/2)*270;
    await destination.mouse.click(x,y);
    await destination.keyboard.press("Control+v");
    await destination.waitForFunction(n=>window.editor.getSceneElements().length===n,index<3?34*(index+1):102+paints.length);
  }
  const composed=await destination.evaluate(()=>window.editor.getSceneElements());
  assert.equal(new Set(composed.map(e=>e.id)).size,102+paints.length);
  const outerGroups=new Set(composed.map(e=>e.groupIds.at(-1)));
  assert.equal(outerGroups.size,4,"independent exports and duplicate paste need distinct outer groups");
  for (const outer of outerGroups) {
    const chart=composed.filter(e=>e.groupIds.at(-1)===outer);
    if (chart.length===paints.length) {
      assert.deepEqual(chart.map(e=>[e.type,e.fillStyle,e.opacity,e.backgroundColor,e.strokeColor]),paintsRaw.map(e=>[e.type,e.fillStyle,e.opacity,e.backgroundColor,e.strokeColor]));
      continue;
    }
    assert.equal(chart.length,34);
    assert.equal(chart[0].type,"rectangle");
    assert.equal(chart[0].backgroundColor,"#ffffff");
    assert.equal(chart[0].width,800);
    assert.equal(chart[0].height,440);
    const inner=new Set(chart.filter(e=>e.groupIds.length===2).map(e=>e.groupIds[0]));
    assert.equal(inner.size,2);
    for (const group of inner) assert.equal(chart.filter(e=>e.groupIds[0]===group).length,3);
  }
  const innerGroups=new Set(composed.filter(e=>e.groupIds.length===2).map(e=>e.groupIds[0]));
  assert.equal(innerGroups.size,6,"series groups must not leak across pasted charts");
  for (const [index,json] of [documents.clean,documents["sketch-1"],documents.clean].entries()) {
    const source=JSON.parse(json).elements;
    const chart=composed.slice(index*34,(index+1)*34);
    const dx=chart[0].x-source[0].x, dy=chart[0].y-source[0].y;
    for (const [i,e] of chart.entries()) {
      assert.ok(Math.abs(e.x-source[i].x-dx)<1e-8 && Math.abs(e.y-source[i].y-dy)<1e-8);
      for (const field of ["type","width","height","angle","points","text","strokeColor","backgroundColor","opacity","roughness","fillStyle"]) assert.deepEqual(e[field],source[i][field], `copy ${index}, element ${i}, ${field}`);
      assert.ok(Number.isInteger(e.seed) && e.seed>0);
    }
  }
  const saved=await destination.evaluate(()=>window.checks.save());
  const reopened=await destination.evaluate(json=>window.checks.load(json),saved);
  for (const [i,e] of reopened.entries()) {
    for (const field of ["id","type","groupIds","x","y","width","height","points","strokeColor","backgroundColor","opacity","fillStyle","roughness","seed"]) assert.deepEqual(e[field],composed[i][field]);
  }
  assert.equal(await destination.evaluate(()=>window.editor.getAppState().viewBackgroundColor),"#d3f9d8");
  await destination.evaluate(()=>window.editor.updateScene({appState:{scrollX:20,scrollY:100,zoom:{value:0.5}}}));
  await writeFile(path.join(results,"composition.excalidraw"),saved);
  await destination.screenshot({path:path.join(results,"composition.png")});
  await destination.close();
  return {elements:raw.length,seriesMembers:members.length,composedCharts:3,paintElements:paints.length,composedElements:composed.length};
}
