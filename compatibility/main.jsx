import React from "react";
import { createRoot } from "react-dom/client";
import { Excalidraw, loadFromBlob, restoreElements, serializeAsJSON, exportToSvg, exportToBlob } from "@excalidraw/excalidraw";
import "@excalidraw/excalidraw/index.css";
import fontUrl from "./node_modules/@excalidraw/excalidraw/dist/prod/fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2?url";

// Resolve the package's own assets locally, independent of CDN availability.
window.EXCALIDRAW_ASSET_PATH = "/node_modules/@excalidraw/excalidraw/dist/prod/";
const font = new FontFace("Excalifont", `url(${fontUrl})`);
document.fonts.add(await font.load());
await document.fonts.ready;
window.checks = {
  measure(text, size) {
    const ctx = document.createElement("canvas").getContext("2d");
    ctx.font = `${size}px Excalifont`;
    return ctx.measureText(text).width;
  },
  async load(json) {
    const scene = await loadFromBlob(new Blob([json], { type: "application/json" }), null, null);
    window.editor.updateScene({ elements: scene.elements, appState: { ...scene.appState, scrollX: 100, scrollY: 100, zoom: { value: 1 } } });
    return scene.elements;
  },
  // In 0.18.0, restoreElements returns early without repairBindings, skipping
  // refreshDimensions entirely. Callers pass the complete scene to retain frames.
  refresh(elements) { return restoreElements(elements, null, { repairBindings: true, refreshDimensions: true }); },
  save() { return serializeAsJSON(window.editor.getSceneElements(), window.editor.getAppState(), {}, "local"); },
  async importLibrary(json) {
    return window.editor.updateLibrary({ libraryItems: new Blob([json], { type: "application/vnd.excalidrawlib+json" }), merge: true, openLibraryMenu: true });
  },
  async png() {
    const blob = await exportToBlob({ elements: window.editor.getSceneElements(), appState: { ...window.editor.getAppState(), exportBackground: false }, files: {}, mimeType: "image/png" });
    return Array.from(new Uint8Array(await blob.arrayBuffer()));
  },
  async svg(ids) { return (await exportToSvg({elements:window.editor.getSceneElements().filter(e => !ids || ids.includes(e.id)), appState:{...window.editor.getAppState(),exportBackground:false}, files:{}})).outerHTML; },
  async frameSvg(id) { return (await exportToSvg({elements:window.editor.getSceneElements(), appState:{...window.editor.getAppState(),exportBackground:false}, files:{}, exportingFrame:window.editor.getSceneElements().find(e=>e.id===id)})).outerHTML; },
};
createRoot(document.getElementById("root")).render(
  <Excalidraw excalidrawAPI={(api) => { window.editor = api; }} />
);
