import React from "react";
import { createRoot } from "react-dom/client";
import * as excalidraw from "@excalidraw/excalidraw";

window.EXCALIDRAW_EXPORT_SOURCE = "document-oracle";
const snapshot = import.meta.env.VITE_DOCUMENT_SNAPSHOT === "true";
if (!snapshot) {
  await import("@excalidraw/excalidraw/index.css");
  window.EXCALIDRAW_ASSET_PATH = "/node_modules/@excalidraw/excalidraw/dist/prod/";
}
window.oracle = {
  serialize: (input, mode) => JSON.parse(excalidraw.serializeAsJSON(input.elements, input.appState, input.files, mode)),
  load: async input => {
    const scene = await excalidraw.loadFromBlob(new Blob([JSON.stringify(input)], {type:"application/json"}), null, null);
    return {elements:scene.elements, appState:scene.appState, files:scene.files};
  },
  library: items => JSON.parse(excalidraw.serializeLibraryAsJSON(items)),
  loadLibrary: input => excalidraw.loadLibraryFromBlob(new Blob([JSON.stringify(input)], {type:"application/json"})),
  async loadEmbedded({png,svg}) {
    const bytes=Uint8Array.from(atob(png),c=>c.charCodeAt(0));
    const image=await excalidraw.loadFromBlob(new Blob([bytes],{type:"image/png"}),null,null);
    const vector=await excalidraw.loadFromBlob(new Blob([svg],{type:"image/svg+xml"}),null,null);
    return {png:{elements:image.elements,files:image.files},svg:{elements:vector.elements,files:vector.files}};
  },
  async exportEmbedded() {
    const options={elements:window.editor.getSceneElements(),appState:{...window.editor.getAppState(),exportEmbedScene:true,exportBackground:true},files:window.editor.getFiles()};
    const png=await excalidraw.exportToBlob({...options,mimeType:"image/png"});
    const bytes=new Uint8Array(await png.arrayBuffer());
    let binary="";for(const byte of bytes)binary+=String.fromCharCode(byte);
    const svg=await excalidraw.exportToSvg(options);
    return {png:btoa(binary),svg:svg.outerHTML};
  },
  restore: elements => excalidraw.restoreElements(elements, null, {repairBindings:true}),
  async mount(input) {
    if (window.reactRoot) window.reactRoot.unmount();
    window.editor = null;
    const scene = await this.load(input);
    if (excalidraw.Fonts) await excalidraw.Fonts.loadElementsFonts(scene.elements);
    window.reactRoot = createRoot(document.getElementById("root"));
    const callback = api => { window.editor = api; };
    window.reactRoot.render(<excalidraw.Excalidraw initialData={scene} {...(snapshot ? {onExcalidrawAPI:callback} : {excalidrawAPI:callback})} />);
    return scene;
  },
  save() {
    return JSON.parse(excalidraw.serializeAsJSON(window.editor.getSceneElementsIncludingDeleted(), window.editor.getAppState(), window.editor.getFiles(), "local"));
  },
};
