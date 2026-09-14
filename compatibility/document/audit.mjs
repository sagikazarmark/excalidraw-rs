// Independent schema drift check against TypeScript declarations, including the
// line/elbow refinements absent from the top-level element union.
import {createRequire} from "node:module";
import {readFile} from "node:fs/promises";
import {execFileSync} from "node:child_process";
import path from "node:path";
import assert from "node:assert/strict";
const source=process.env.EXCALIDRAW_SOURCE;
if(!source) throw new Error("EXCALIDRAW_SOURCE must name the pinned checkout with dependencies installed");
assert.equal(execFileSync("git",["rev-parse","HEAD"],{cwd:source,encoding:"utf8"}).trim(),"afa3a653fc5d2b742adcbd5a6063187b056d2419");
const require=createRequire(path.join(source,"package.json"));
const ts=require("typescript");
const rust=await readFile(path.resolve(import.meta.dirname,"../../crates/excalidraw-document/src/model.rs"),"utf8");
const fields=new Set([...rust.matchAll(/=>\s*"([A-Za-z][A-Za-z0-9]*)"/g)].map(m=>m[1]));
for(const profile of ["snapshot","release"]) {
  const text=profile==="snapshot"?await readFile(path.join(source,"packages/element/src/types.ts"),"utf8"):
    process.env.EXCALIDRAW_RELEASE_TYPES?await readFile(process.env.EXCALIDRAW_RELEASE_TYPES,"utf8"):
    execFileSync("git",["show","v0.18.1:packages/excalidraw/element/types.ts"],{cwd:source,encoding:"utf8"});
  const file=ts.createSourceFile("types.ts",text,ts.ScriptTarget.Latest,true);
  const names=new Set(["_ExcalidrawElementBase","ExcalidrawRectangleElement","ExcalidrawDiamondElement","ExcalidrawEllipseElement","ExcalidrawTextElement","ExcalidrawLinearElement","ExcalidrawLineElement","ExcalidrawArrowElement","ExcalidrawElbowArrowElement","ExcalidrawFreeDrawElement","ExcalidrawImageElement","ImageCrop","ExcalidrawFrameElement","ExcalidrawMagicFrameElement","ExcalidrawIframeElement","ExcalidrawEmbeddableElement","ExcalidrawStickyNoteElement","FixedPointBinding","PointBinding","FixedSegment","StrokeOptions","BoundElement"]);
  const properties=new Set();
  for(const node of file.statements) if(ts.isTypeAliasDeclaration(node)&&names.has(node.name.text)) {
    const walk=n=>{if(ts.isPropertySignature(n)&&n.name) properties.add(n.name.getText(file)); ts.forEachChild(n,walk);}; walk(node.type);
  }
  // generationData is an open customData subrecord, exposed through GenerationData.
  properties.delete("generationData");
  properties.delete("_brand"); // TypeScript-only numeric/string brands have no wire field.
  assert.deepEqual([...properties].filter(p=>!fields.has(p)),[],`${profile} missing typed descriptors`);
  console.log(`${profile}: audited ${properties.size} distinct declared property names`);
}
