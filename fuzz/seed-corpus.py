"""Populate a separate fuzz corpus without changing the committed fixtures."""

import argparse
import hashlib
import json
import base64
import struct
import zlib
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("destination", type=Path)
args = parser.parse_args()
args.destination.mkdir(parents=True, exist_ok=True)
root = Path(__file__).resolve().parents[1]


def seed(data):
    path = args.destination / hashlib.sha256(data).hexdigest()
    if not path.exists():
        path.write_bytes(data)


for path in (root / "crates/excalidraw-document/tests/fixtures").glob("*.json"):
    seed(path.read_bytes())

# Reach valid document and validation branches early, alongside malformed input.
for value in [
    {"type": "excalidraw"},
    {"type": "excalidraw", "elements": [None, {"type": "future"}]},
    {"type": "excalidraw", "elements": [{"type": "arrow", "elbowed": True,
        "points": [[0, 0], [10, 0], [10, 20], [30, 20]],
        "fixedSegments": [{"index": 2, "start": [10, 0], "end": [10, 20]}]}]},
    {"type": "excalidraw", "elements": [{"type": "image", "fileId": "a",
        "crop": {"x": 0, "y": 0, "width": 1, "height": 1,
                 "naturalWidth": 2, "naturalHeight": 2}}], "files": {"a": {}}},
    {"type": "excalidraw", "elements": [{"type": "text", "id": "a", "containerId": "b"},
        {"type": "rectangle", "id": "b", "boundElements": [{"id": "a", "type": "text"}]}]},
    {"type": "excalidraw", "elements": [{"type": "freedraw", "points": [[0, 0]],
        "pressures": [0.5], "simulatePressure": False}]},
    {"type": "excalidrawlib", "version": 1, "library": [[{"type": "rectangle"}]]},
    {"type": "excalidrawlib", "version": 2, "libraryItems": [{"elements": []}]},
]:
    seed(json.dumps(value).encode())

seed(b'{"type":"excalidraw","extension":1e400}')
seed(b'{"type":"excalidraw","extension":{"$serde_json::private::Number":"123"}}')
seed(b'{"type":"excalidraw","elements":[],"elements":null}')
scene=b'{"type":"excalidraw","elements":[],"source":"fuzz-embedded"}'
seed(b'<svg><metadata><!-- payload-type:application/vnd.excalidraw+json --><!-- payload-start -->'+base64.b64encode(scene)+b'<!-- payload-end --></metadata></svg>')
payload=json.dumps({"version":"1","encoding":"bstring","compressed":True,"encoded":zlib.compress(scene).decode("latin1")}).encode("ascii")
def chunk(kind,data):
    return struct.pack(">I",len(data))+kind+data+struct.pack(">I",zlib.crc32(kind+data))
seed(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',1,1,8,6,0,0,0))+chunk(b'tEXt',b'application/vnd.excalidraw+json\0'+payload)+chunk(b'IDAT',zlib.compress(bytes([0,255,0,0,255])))+chunk(b'IEND',b''))
seed(b'{"type":"excalidraw","version":2,"source":"test","elements":[],"appState":{"viewBackgroundColor":"white"},"files":{},"sceneVersion":"v"}')
print(f"Seeded corpus in {args.destination}")
