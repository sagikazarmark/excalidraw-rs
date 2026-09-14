// All scene geometry, resources, relationships and library items originate in Rust.
// JavaScript only drives the native editor and asserts observable behavior.
import assert from "node:assert/strict";
import {runRust} from "./rust.mjs";

const find = (scene,id) => {
  const element=scene.elements.find(e=>e.id===id);
  assert.ok(element,`missing ${id}`);
  return element;
};

export async function authoredGallery(page,repo,snapshot,artifact) {
  const input=runRust(repo, "authored_gallery", [snapshot ? "snapshot" : "release"]);
  await artifact("input",input);
  const checks={};
  // Check every authored field, not just types after normalization. Only empty
  // reverse bindings permit null <-> []. Text geometry is exact at every boundary,
  // including native save/reopen after editing; neither pinned font needs exceptions.
  const invariants=(expected,actual,stage)=>{
    assert.deepEqual(actual.elements.map(e=>e.id),expected.elements.map(e=>e.id),`${stage}: order/identity`);
    for(const e of expected.elements) {
      const a=find(actual,e.id);
      for(const [key,value] of Object.entries(e)) {
        if(key==="boundElements" && (value===null || Array.isArray(value))) {
          assert.deepEqual(a[key]===null?[]:a[key],value===null?[]:value,`${stage}: ${e.id}.${key}`);
        } else assert.deepEqual(a[key],value,`${stage}: ${e.id}.${key}`);
      }
    }
    assert.deepEqual(actual.files,expected.files,`${stage}: resources`);
  };
  const load=async (doc,name)=>{
    const loaded=await page.evaluate(doc=>window.oracle.load(doc),doc);
    await artifact(`${name}-loaded`,loaded);
    invariants(doc,loaded,`${name} load`);
    return loaded;
  };
  const save=()=>page.evaluate(()=>window.oracle.save());
  const mount=async (doc,name)=>{
    await load(doc,name);
    await page.evaluate(doc=>window.oracle.mount(doc),doc);
    await page.waitForFunction(ids=>window.editor && ids.every(id=>window.editor.getSceneElements().some(e=>e.id===id)),doc.elements.map(e=>e.id));
    await page.waitForFunction(ids=>ids.every(id=>window.editor.getFiles()[id]),Object.keys(doc.files));
    await page.evaluate(()=>window.editor.updateScene({appState:{scrollX:0,scrollY:0,zoom:{value:1}}}));
    await page.waitForFunction(()=>window.editor.getAppState().scrollX===0 && window.editor.getAppState().zoom.value===1);
    await page.mouse.click(1000,700);
    const mounted=await save();
    await artifact(`${name}-mounted`,mounted);
    invariants(doc,mounted,`${name} mount`);
    return mounted;
  };
  const reopen=async name=>{
    const saved=await save();
    await artifact(`${name}-saved`,saved);
    const reopened=await load(saved,`${name}-reopened`);
    return {saved,reopened};
  };
  const get=id=>page.evaluate(id=>window.editor.getSceneElements().find(e=>e.id===id),id);
  const position=async(id,fx=0.5,fy=0.5)=>page.evaluate(({id,fx,fy})=>{
    const e=window.editor.getSceneElements().find(e=>e.id===id),s=window.editor.getAppState();
    return [(e.x+e.width*fx+s.scrollX)*s.zoom.value+s.offsetLeft,(e.y+e.height*fy+s.scrollY)*s.zoom.value+s.offsetTop];
  },{id,fx,fy});
  const drag=async(from,to)=>{
    await page.mouse.move(...from);await page.mouse.down();await page.mouse.move(...to,{steps:6});await page.mouse.up();
  };
  const editText=async(id,content)=>{
    await page.mouse.dblclick(...await position(id));
    const textarea=page.locator("textarea.excalidraw-wysiwyg");
    await textarea.waitFor();
    const previous=await textarea.inputValue();
    await textarea.fill(content);await textarea.press("Escape");
    await page.waitForFunction(({id,content})=>window.editor.getSceneElements().find(e=>e.id===id).originalText===content,{id,content});
    return previous;
  };

  const kinds=["rectangle","diamond","ellipse","text","line","arrow","freedraw","image","frame","magicframe","iframe","embeddable",...(snapshot?["stickynote"]:[])];
  assert.deepEqual(input.gallery.elements.map(e=>e.type),kinds);
  const gallery=await load(input.gallery,"gallery");
  // Public native serialization/load also covers inert embed and AI-frame data.
  const gallerySaved=await page.evaluate(doc=>window.oracle.serialize(doc,"local"),gallery);
  await artifact("gallery-native-saved",gallerySaved);
  invariants(input.gallery,gallerySaved,"gallery native serialization");
  await load(gallerySaved,"gallery-reopened");
  await mount(input.interactive,"interactive");
  await reopen("interactive");
  checks.persistedKinds=kinds;
  checks.remoteDataOnly=true;

  assert.deepEqual(input.historicalValidation,{inspect:true,author:true,preservingRoundtrip:true});
  const historicalInvariants=(doc,retainsDeleted,stage)=>{
    assert.deepEqual(doc.elements.map(e=>e.id),retainsDeleted?["historical-box","deleted-label"]:["historical-box"],`${stage}: tombstone presence`);
    const box=find(doc,"historical-box");
    assert.equal(box.isDeleted,false,`${stage}: box must remain live`);
    assert.deepEqual(box.boundElements,[],`${stage}: deleted label reverse edge resurrected`);
    if(retainsDeleted) {
      const label=find(doc,"deleted-label");
      assert.equal(label.isDeleted,true,`${stage}: label resurrected`);
      assert.equal(label.containerId,box.id,`${stage}: historical relationship lost`);
    }
  };
  historicalInvariants(input.historical,true,"Rust preserving output");
  const historicalRestored=await page.evaluate(doc=>({elements:window.oracle.restore(doc.elements),files:doc.files}),input.historical);
  await artifact("historical-restored",historicalRestored);
  invariants(input.historical,historicalRestored,"historical public restoration");
  historicalInvariants(historicalRestored,true,"historical public restoration");
  // Release loadFromBlob calls clearElementsForExport *before* restoration,
  // pruning tombstones on file load as well as save. Snapshot calls restoreElements
  // directly. Keep file-load evidence distinct from public element restoration.
  const historicalExpected=snapshot?input.historical:{...input.historical,elements:[find(input.historical,"historical-box")]};
  const historicalLoaded=await page.evaluate(doc=>window.oracle.load(doc),input.historical);
  await artifact("historical-loaded",historicalLoaded);
  invariants(historicalExpected,historicalLoaded,"historical blob load");
  historicalInvariants(historicalLoaded,snapshot,"historical blob load");
  // Serialize restored history directly, proving that release save prunes a
  // tombstone actually present in the input (rather than one already lost on load).
  const historicalSerialized=await page.evaluate(doc=>window.oracle.serialize(doc,"local"),{...input.historical,elements:historicalRestored.elements});
  await artifact("historical-native-serialized",historicalSerialized);
  invariants(historicalExpected,historicalSerialized,"historical native serialization");
  historicalInvariants(historicalSerialized,snapshot,"historical native serialization");
  const historicalSerializedReopened=await load(historicalSerialized,"historical-native-serialized-reopened");
  historicalInvariants(historicalSerializedReopened,snapshot,"historical serialized reopen");
  await page.evaluate(doc=>window.oracle.mount(doc),input.historical);
  await page.waitForFunction(count=>window.editor?.getSceneElementsIncludingDeleted().length===count,snapshot?2:1);
  const historicalMounted=await page.evaluate(()=>({elements:window.editor.getSceneElementsIncludingDeleted(),files:window.editor.getFiles()}));
  await artifact("historical-mounted",historicalMounted);
  invariants(historicalExpected,historicalMounted,"historical mount");
  historicalInvariants(historicalMounted,snapshot,"historical mount");
  const historicalResult=await reopen("historical");
  for(const [stage,doc] of Object.entries(historicalResult)) {
    historicalInvariants(doc,snapshot,`historical ${stage}`);
    // The only element-level save difference permitted here is release pruning.
    invariants(historicalExpected,doc,`historical ${stage}`);
  }
  checks.historical={...input.historicalValidation,publicRestorationRetainsDeletedRelationship:true,reverseEdgeNotResurrected:true,
    blobLoad:snapshot?"retains-deleted-label":"prunes-deleted-label",
    nativeSave:snapshot?"retains-deleted-label":"prunes-deleted-label",saveReopen:true};

  await mount(input.image,"image");
  const decoded=await page.evaluate(async()=>{
    const file=window.editor.getFiles()["gallery-pixel"];
    const image=new Image();image.src=file.dataURL;await image.decode();
    return [image.naturalWidth,image.naturalHeight];
  });
  assert.deepEqual(decoded,[1,1]);
  await page.mouse.click(...await position("image-subject"));
  await page.keyboard.press("Shift+H");
  await page.waitForFunction(()=>window.editor.getSceneElements()[0].scale[0]===-1);
  await page.mouse.dblclick(...await position("image-subject"));
  await page.waitForFunction(()=>window.editor.getAppState().croppingElementId==="image-subject");
  const left=await position("image-subject",0,0.5);
  await drag(left,[left[0]+40,left[1]]);
  await page.keyboard.press("Escape");
  const cropped=await get("image-subject");
  assert.ok(cropped.crop && cropped.crop.width<cropped.crop.naturalWidth,"native crop did not reduce image");
  const imageResult=await reopen("image");
  assert.deepEqual(find(imageResult.reopened,"image-subject").crop,cropped.crop);
  assert.deepEqual(find(imageResult.reopened,"image-subject").scale,[-1,1]);
  assert.deepEqual(imageResult.reopened.files,input.image.files);
  checks.image={resourceRegistration:true,decode:decoded,nativeFlip:true,nativeCrop:true,saveReopen:true};

  await mount(input.freehand,"freehand");
  await page.mouse.click(...await position("free-subject",0.5,1));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds["free-subject"]);
  const freeBefore=await get("free-subject"),corner=await position("free-subject",1,1);
  await drag(corner,[corner[0]+40,corner[1]+40]);
  await page.waitForFunction(points=>JSON.stringify(window.editor.getSceneElements()[0].points)!==points,JSON.stringify(freeBefore.points));
  const freeResult=await reopen("freehand"),freeAfter=find(freeResult.reopened,"free-subject");
  assert.equal(freeAfter.type,"freedraw");assert.ok(freeAfter.width>freeBefore.width && freeAfter.height>freeBefore.height);
  assert.deepEqual(freeAfter.pressures,freeBefore.pressures);assert.equal(freeAfter.simulatePressure,false);
  checks.freehand={nativeResize:true,samplesChanged:true,pressuresPreserved:true,saveReopen:true};

  await mount(input.multiline,"multiline");
  const editedText="Native first line\nRust β retained\nNative third line";
  assert.equal(await editText("multiline",editedText),find(input.multiline,"multiline").originalText);
  const multilineResult=await reopen("multiline");
  assert.equal(find(multilineResult.reopened,"multiline").text,editedText);
  assert.equal(find(multilineResult.reopened,"multiline").originalText,editedText);
  checks.multiline={nativeEdit:true,saveReopen:true};

  const fixedSegmentConstraint=(arrow,stage)=>{
    assert.equal(arrow.fixedSegments?.length,1,`${stage}: expected one fixed segment`);
    const segment=arrow.fixedSegments[0];
    assert.equal(segment.index,2,`${stage}: authored segment index changed`);
    assert.equal(segment.start[0],100,`${stage}: vertical constraint start x changed`);
    assert.equal(segment.end[0],100,`${stage}: vertical constraint end x changed`);
    assert.ok(segment.end[1]>segment.start[1],`${stage}: fixed segment collapsed or reversed`);
    // Upstream math pointsEqual uses coordinate-wise PRECISION=10e-5.
    // Routing may change endpoint y (e.g. 80 -> 80.008), but the segment must
    // still describe the corresponding routed samples, not stale authored ones.
    for(const [endpoint,index] of [[segment.start,segment.index-1],[segment.end,segment.index]]) {
      const point=arrow.points[index];
      assert.ok(point,`${stage}: segment index outside path`);
      for(let axis=0;axis<2;axis++)assert.ok(Math.abs(endpoint[axis]-point[axis])<1e-4,`${stage}: segment endpoint differs from points[${index}][${axis}]`);
    }
  };
  fixedSegmentConstraint(find(input.elbow,"elbow"),"Rust elbow");
  await mount(input.elbow,"elbow");
  await page.mouse.click(...await position("target"));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.target);
  const elbowBefore=await get("elbow"),targetBefore=await get("target");
  await page.keyboard.press("Shift+ArrowRight");
  await page.waitForFunction(x=>window.editor.getSceneElements().find(e=>e.id==="target").x>x,targetBefore.x);
  await page.waitForFunction(points=>JSON.stringify(window.editor.getSceneElements().find(e=>e.id==="elbow").points)!==points,JSON.stringify(elbowBefore.points));
  const elbowResult=await reopen("elbow"),elbowAfter=find(elbowResult.reopened,"elbow");
  assert.equal(elbowAfter.elbowed,true);assert.equal(elbowAfter.endBinding.elementId,"target");
  fixedSegmentConstraint(find(elbowResult.saved,"elbow"),"routed elbow save");
  fixedSegmentConstraint(elbowAfter,"routed elbow reopen");
  assert.deepEqual(find(elbowResult.reopened,"target").boundElements,[{id:"elbow",type:"arrow"}]);
  checks.elbow={nativeTargetMove:true,rerouted:true,fixedSegmentsPreserved:true,saveReopen:true};

  const isolation=doc=>{
    assert.equal(new Set(doc.elements.map(e=>e.id)).size,12);
    assert.equal(Object.keys(doc.files).length,3);
    for(const prefix of ["","copy-","library-"]) {
      const box=find(doc,`${prefix}box`),label=find(doc,`${prefix}label`),asset=find(doc,`${prefix}asset`);
      assert.equal(label.containerId,box.id);
      assert.deepEqual(box.boundElements,[{id:label.id,type:"text"}]);
      for(const e of [box,label,asset]) {
        assert.equal(e.frameId,`${prefix}frame`);
        assert.deepEqual(e.groupIds,[`${prefix}cards`]);
      }
      const fileId=prefix?`${prefix}pixel`:"gallery-pixel";
      assert.equal(asset.fileId,fileId);assert.equal(doc.files[fileId].id,fileId);
      assert.equal(doc.files[fileId].dataURL,input.image.files["gallery-pixel"].dataURL);
    }
  };
  isolation(input.composition);
  // Independently check the Rust translation and copy offsets, rather than
  // merely trusting that the native loader retained whatever Rust emitted.
  assert.equal(find(input.composition,"frame").x,90);
  assert.equal(find(input.composition,"frame").y,180);
  assert.equal(find(input.composition,"box").x,120);
  assert.equal(find(input.composition,"box").y,220);
  for(const [prefix,offset] of [["copy-",300],["library-",600]]) {
    for(const id of ["frame","box","label","asset"]) {
      const original=find(input.composition,id),copy=find(input.composition,`${prefix}${id}`);
      assert.equal(copy.x,original.x+offset);
      for(const key of ["y","width","height","seed","version","updated"])assert.equal(copy[key],original[key]);
    }
  }
  // set_frame normalizes children below their frame; duplication and reordering
  // preserve each complete block, including the bound label beside its box.
  const compositionOrder=["library-box","library-label","library-asset","library-frame","box","label","asset","frame","copy-box","copy-label","copy-asset","copy-frame"];
  assert.deepEqual(input.composition.elements.map(e=>e.id),compositionOrder);
  const compositionBefore=await mount(input.composition,"composition");
  assert.deepEqual(compositionBefore.elements.map(e=>e.id),compositionOrder);
  isolation(compositionBefore);
  // Select the frame's border with a real pointer, then move its descendants.
  await page.mouse.click(...await position("frame",0.5,0));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds.frame);
  await page.keyboard.press("Shift+ArrowDown");
  await page.waitForFunction(y=>window.editor.getSceneElements().find(e=>e.id==="frame").y>y,find(compositionBefore,"frame").y);
  const frameMoved=await save();
  const frameDelta=find(frameMoved,"frame").y-find(compositionBefore,"frame").y;
  for(const id of ["box","label","asset"])assert.equal(find(frameMoved,id).y-find(compositionBefore,id).y,frameDelta,`${id} did not follow frame`);
  for(const e of compositionBefore.elements.filter(e=>e.id.startsWith("copy-")||e.id.startsWith("library-")))assert.deepEqual(find(frameMoved,e.id),e,`frame move leaked into ${e.id}`);
  await artifact("composition-frame-moved",frameMoved);
  await reopen("composition-frame");
  await page.keyboard.press("Escape");
  // Click the image in the duplicate's Rust-authored group, away from labels.
  await page.mouse.click(...await position("copy-asset"));
  await page.waitForFunction(()=>window.editor.getAppState().selectedElementIds["copy-box"] && window.editor.getAppState().selectedElementIds["copy-asset"]);
  const groupBefore=await save();
  await page.keyboard.press("Shift+ArrowRight");
  await page.waitForFunction(x=>window.editor.getSceneElements().find(e=>e.id==="copy-box").x>x,find(groupBefore,"copy-box").x);
  const groupMoved=await save(),groupDelta=find(groupMoved,"copy-box").x-find(groupBefore,"copy-box").x;
  for(const id of ["copy-label","copy-asset"])assert.equal(find(groupMoved,id).x-find(groupBefore,id).x,groupDelta,`${id} did not follow group`);
  for(const e of groupBefore.elements.filter(e=>!e.id.startsWith("copy-")||e.id==="copy-frame"))assert.deepEqual(find(groupMoved,e.id),e,`group move leaked into ${e.id}`);
  const compositionResult=await reopen("composition-group");
  isolation(compositionResult.reopened);
  checks.composition={rustFrameGroupUngroupTranslateReorder:true,duplicateIdentityIsolation:true,libraryInsertionIdentityIsolation:true,nativeFrameMove:true,nativeGroupMove:true,saveReopen:true};

  assert.equal(input.library.version,2);
  const library=await page.evaluate(doc=>window.oracle.loadLibrary(doc),input.library);
  await artifact("library-loaded",library);
  assert.equal(library.length,1);
  for(const key of ["id","status","created"])assert.deepEqual(library[0][key],input.library.libraryItems[0][key]);
  invariants({elements:input.library.libraryItems[0].elements,files:{}},{elements:library[0].elements,files:{}},"library blob load");
  const librarySaved=await page.evaluate(items=>window.oracle.library(items),library);
  await artifact("library-saved",librarySaved);
  const libraryReopened=await page.evaluate(doc=>window.oracle.loadLibrary(doc),librarySaved);
  await artifact("library-reopened",libraryReopened);
  assert.deepEqual(libraryReopened,library);
  checks.libraryV2={nativeBlobLoad:true,nativeSerializeReload:true};

  if(snapshot) {
    await mount(input.sticky,"sticky");
    const before=await get("sticky");
    await editText("sticky-label",Array.from({length:20},(_,i)=>`Line ${i}`).join("\n"));
    const grown=await get("sticky");assert.ok(grown.height>before.height,"sticky did not grow");
    await reopen("sticky-grown");
    await editText("sticky-label","Short note");
    const shrunk=await get("sticky");assert.ok(shrunk.height<grown.height,"sticky did not shrink");
    assert.ok(shrunk.height>=shrunk.baseHeight);assert.equal(shrunk.baseHeight,120);
    const stickyResult=await reopen("sticky-shrunk");
    assert.equal(find(stickyResult.reopened,"sticky-label").baseFontSize,20);
    assert.equal(find(stickyResult.reopened,"sticky-label").containerId,"sticky");
    checks.sticky={nativeGrow:true,nativeShrink:true,baseHeight:120,saveReopen:true};
  }
  checks.loadAndReopenInvariants=true;
  checks.exactTextGeometry=true;
  return checks;
}
