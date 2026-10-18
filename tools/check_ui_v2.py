#!/usr/bin/env python3
"""Real Chromium flow for the shared frontend against a deterministic mock API.

This isolates UI state-machine behavior from the DeepSeek service and provides
the same contract names used by the Rust routes. It never starts Unity, OSC or
VRChat. Root can additionally run this script with --url against the real app.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import mimetypes
import threading
import time
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from check_editor_browser import Browser

ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / "web"
OUT = ROOT / "docs/ui-v2"


class MockState:
    def __init__(self):
        self.lock = threading.Lock()
        self.revision = 7
        self.boxes = []
        self.protected_groups = []
        self.run = None
        self.epoch = 0
        self.phase = "idle"
        self.paused = False
        self.queue = []
        self.requests = {}
        self.events = []
        self.seq = 1
        self.preview_calls = 0
        self.apply_calls = 0
        self.last_apply = {}
        self.last_project = {}
        self.edit_calls = []
        self.message_calls = 0
        self.message_delay = 0.12
        self.preview_delay = 0.0

    def event(self, kind, request_id=None, **data):
        value = {
            "seq": self.seq,
            "time": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "kind": kind,
            "runId": self.run["id"] if self.run else None,
            "revision": self.revision,
            "data": data,
        }
        if request_id:
            value["data"]["requestId"] = request_id
            value["data"]["runId"] = self.run["id"] if self.run else None
            value["data"]["epoch"] = self.epoch
        self.seq += 1
        self.events.append(value)

    def state(self):
        return {
            "project": {
                "id": "ui-v2-mock",
                "name": "UI v2 Mock",
                "pinnedNotes": "",
                "protectedGroups": list(self.protected_groups),
            },
            "revision": self.revision,
            "stats": {
                "boxCount": len(self.boxes),
                "voxelCount": sum(
                    box["size"][0] * box["size"][1] * box["size"][2]
                    for box in self.boxes
                ),
                "targetBoxes": 16384,
                "groups": {"main": len(self.boxes)} if self.boxes else {},
                "bounds": None,
            },
            "canUndo": self.revision > 7,
            "canRedo": False,
            "connection": {
                "enabled": False,
                "port": 9012,
                "profile": "mf3",
                "interval": 0.25,
                "capacity": 16384,
                "refreshSeconds": 0,
                "auto": False,
                "pending": 0,
                "error": "",
                "desiredRevision": self.revision,
                "sentRevision": None,
                "sentCommands": 0,
                "etaSeconds": 0,
                "placement": {
                    "world": True,
                    "visible": True,
                    "level": True,
                    "scale": 1.0,
                },
            },
            "harness": {
                "settings": {
                    "model": "deepseek-flash",
                    "reasoning": "high",
                    "maxSteps": 24,
                    "maxRunSeconds": 900,
                    "maxRunTokens": 250000,
                    "maxOutputTokens": 16384,
                    "compactTokens": 60000,
                    "autoCompact": True,
                    "language": "zh-CN",
                },
                "provider": {"configured": True},
                "run": self.run,
                "paused": self.paused,
                "interaction": {
                    "phase": self.phase,
                    "epoch": self.epoch,
                    "runId": self.run["id"] if self.run else "",
                    "activeRequestId": "",
                    "paused": self.paused,
                    "queue": list(self.queue),
                },
                "context": {
                    "estimatedTokens": 120,
                    "messages": 2,
                    "compactCount": 0,
                    "summary": "",
                },
            },
        }


MOCK = MockState()


def json_bytes(value):
    return json.dumps(value, ensure_ascii=False).encode("utf-8")


def mock_text_geometry(body):
    origin = body.get("origin", [0, 0, 0])
    depth = int(body.get("depth", 1))
    plane = body.get("plane", "xy")
    if str(body.get("text", "")).strip().upper() == "L":
        width, height = 6, 5
        rectangles = ((0, 1, 0, 4), (1, 6, 4, 5))
    else:
        width, height = 6, 3
        rectangles = ((0, 6, 0, 1), (0, 2, 1, 2), (0, 4, 2, 3))
    operations = []
    for u0, u1, v0, v1 in rectangles:
        minimum = list(origin)
        size = [0, 0, 0]
        if plane == "yz":
            minimum[2] = origin[2] + u0
            size[2] = u1 - u0
            minimum[1] = origin[1] + height - v1
            size[1] = v1 - v0
            size[0] = depth
        elif plane == "xz":
            minimum[0] = origin[0] + width - u1
            size[0] = u1 - u0
            minimum[2] = origin[2] + v0
            size[2] = v1 - v0
            size[1] = depth
        else:
            minimum[0] = origin[0] + width - u1
            size[0] = u1 - u0
            minimum[1] = origin[1] + height - v1
            size[1] = v1 - v0
            size[2] = depth
        operations.append(
            {
                "op": "box",
                "min": minimum,
                "size": size,
                "styleId": body.get("styleId", 1),
                "tintId": body.get("tintId", 0),
                "motionPhase": body.get("motionPhase", 0),
                "group": body.get("group", "text"),
            }
        )
    lo = [min(op["min"][axis] for op in operations) for axis in range(3)]
    hi = [
        max(op["min"][axis] + op["size"][axis] for op in operations)
        for axis in range(3)
    ]
    voxels = sum(
        op["size"][0] * op["size"][1] * op["size"][2]
        for op in operations
    )
    return operations, {
        "width": width,
        "height": height,
        "depth": depth,
        "voxelCount": voxels,
        "boxCount": len(operations),
        "bounds": {"min": lo, "max": hi},
        "missingGlyphs": [],
    }


class Handler(BaseHTTPRequestHandler):
    server_version = "MorphForgeUIV2/1"

    def log_message(self, *_args):
        return

    def send_json(self, value, status=200):
        body = json_bytes(value)
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def read_json(self):
        length = int(self.headers.get("Content-Length", "0"))
        return json.loads(self.rfile.read(length) or b"{}")

    def do_GET(self):
        with MOCK.lock:
            if self.path == "/api/state":
                return self.send_json(MOCK.state())
            if self.path == "/api/scene":
                return self.send_json(
                    {
                        "scene": {
                            "revision": MOCK.revision,
                            "cellSize": 0.25,
                            "size": 256,
                            "boxes": MOCK.boxes,
                        },
                        "materials": [
                            {
                                "id": 1,
                                "uid": "mf:stone",
                                "name": "石灰岩",
                                "family": "natural",
                                "color": [0.62, 0.67, 0.73],
                                "renderMode": "opaque",
                                "textures": [],
                            },
                            {
                                "id": 2,
                                "uid": "mf:oak",
                                "name": "暖橡木",
                                "family": "wood",
                                "color": [0.64, 0.35, 0.16],
                                "renderMode": "opaque",
                                "textures": [],
                            },
                        ],
                    }
                )
            if self.path == "/api/fonts":
                return self.send_json(
                    {
                        "fonts": [
                            {
                                "id": "builtin:noto-sans-sc",
                                "family": "Noto Sans SC",
                                "style": "Regular",
                                "source": "builtin",
                                "sha256": "mock",
                                "faceIndex": 0,
                                "license": "OFL-1.1",
                            }
                        ]
                    }
                )
            if self.path.startswith("/api/events"):
                after = 0
                if "after=" in self.path:
                    after = int(self.path.split("after=", 1)[1].split("&", 1)[0])
                values = [value for value in MOCK.events if value["seq"] > after]
                return self.send_json(
                    {
                        "events": values,
                        "lastSeq": MOCK.events[-1]["seq"] if MOCK.events else 0,
                    }
                )
            if self.path.startswith("/api/history"):
                return self.send_json(
                    {"revisions": [MOCK.revision], "commits": []}
                )
            if self.path.startswith("/api/artworks"):
                return self.send_json({"artworks": []})
            if self.path.startswith("/api/capture/jobs"):
                return self.send_json(None)
            if self.path.startswith("/api/metrics"):
                return self.send_json({})
        path = self.path.split("?", 1)[0].lstrip("/") or "studio.html"
        target = (WEB / path).resolve()
        if WEB.resolve() not in target.parents and target != WEB:
            self.send_error(404)
            return
        if not target.is_file():
            self.send_error(404)
            return
        body = target.read_bytes()
        self.send_response(200)
        self.send_header(
            "Content-Type",
            mimetypes.guess_type(target.name)[0] or "application/octet-stream",
        )
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        length = int(self.headers.get("Content-Length", "0"))
        if self.path.startswith("/api/packs/import"):
            self.rfile.read(length)
            return self.send_json({"materialsAdded": 0})
        if self.path == "/mock/advance":
            payload = json.loads(self.rfile.read(length) or b"{}")
            with MOCK.lock:
                MOCK.run = {
                    "id": payload.get("runId", "run-1"),
                    "status": payload.get("status", "running"),
                    "prompt": payload.get("prompt", "启动灯塔"),
                    "steps": 1,
                    "tokens": 42,
                    "message": payload.get("message", ""),
                    "error": "",
                }
                MOCK.epoch = int(payload.get("epoch", MOCK.epoch))
                MOCK.phase = payload.get("phase", "running")
                MOCK.paused = bool(payload.get("paused", False))
                if payload.get("emitFinished") and MOCK.run:
                    MOCK.event(
                        "harness.finished",
                        MOCK.run["id"],
                        **dict(MOCK.run),
                    )
            return self.send_json({"ok": True})
        body = json.loads(self.rfile.read(length) or b"{}")
        with MOCK.lock:
            if self.path == "/api/harness/message":
                return self.handle_message(body)
            if self.path == "/api/harness/control":
                action = body.get("action")
                if action == "pause":
                    MOCK.paused = True
                    MOCK.phase = "paused"
                elif action == "interrupt":
                    MOCK.paused = False
                    MOCK.phase = "interrupting"
                    MOCK.epoch += 1
                    if MOCK.run:
                        MOCK.run["status"] = "interrupting"
                elif action == "resume":
                    MOCK.paused = False
                    MOCK.phase = "running"
                elif action == "stop":
                    MOCK.phase = "stopped"
                    if MOCK.run:
                        MOCK.run["status"] = "stopped"
                return self.send_json(MOCK.state())
            if self.path == "/api/text/preview":
                MOCK.preview_calls += 1
                if MOCK.preview_delay:
                    time.sleep(MOCK.preview_delay)
                if "\U0001f600" in str(body.get("text", "")):
                    return self.send_json(
                        {
                            "ok": False,
                            "code": "FONT_MISSING_GLYPHS",
                            "error": "FONT_MISSING_GLYPHS: U+1F600",
                            "missingGlyphs": ["U+1F600"],
                        },
                        400,
                    )
                operations, layout = mock_text_geometry(body)
                canonical = json.dumps(
                    body, ensure_ascii=False, sort_keys=True, separators=(",", ":")
                ).encode()
                return self.send_json(
                    {
                        "ok": True,
                        "requestId": body.get("requestId"),
                        "revision": MOCK.revision,
                        "requestFingerprint": hashlib.sha256(canonical).hexdigest(),
                        "font": {
                            "id": body.get("fontId"),
                            "family": "Noto Sans SC",
                            "style": "Regular",
                        },
                        "layout": layout,
                        "operations": operations,
                        "warnings": [],
                    }
                )
            if self.path == "/api/text/apply":
                MOCK.apply_calls += 1
                MOCK.last_apply = body
                if body.get("baseRevision") != MOCK.revision:
                    return self.send_json(
                        {"ok": False, "error": "REVISION_CONFLICT"}, 409
                    )
                request_id = body.get("requestId")
                if not request_id:
                    return self.send_json(
                        {"ok": False, "error": "requestId required"}, 400
                    )
                MOCK.revision += 1
                MOCK.boxes = [
                    {
                        "id": index + 1,
                        "min": [body.get("origin", [0, 0, 0])[0] + index * 3, 0, 0],
                        "size": [2, 2, body.get("depth", 1)],
                        "styleId": body.get("styleId", 1),
                        "tintId": body.get("tintId", 0),
                        "motionPhase": body.get("motionPhase", 0),
                        "group": body.get("group", "text"),
                    }
                    for index in range(2)
                ]
                return self.send_json(
                    {
                        "ok": True,
                        "requestId": body.get("requestId"),
                        "requestFingerprint": hashlib.sha256(
                            json.dumps(
                                {
                                    key: value
                                    for key, value in body.items()
                                    if key
                                    not in {
                                        "requestId",
                                        "baseRevision",
                                        "previewRequestId",
                                        "previewFingerprint",
                                    }
                                },
                                ensure_ascii=False,
                                sort_keys=True,
                                separators=(",", ":"),
                            ).encode()
                        ).hexdigest(),
                        "revision": MOCK.revision,
                        "scene": {
                            "revision": MOCK.revision,
                            "boxes": MOCK.boxes,
                        },
                        "stats": MOCK.state()["stats"],
                        "font": {
                            "id": body.get("fontId"),
                            "family": "Noto Sans SC",
                            "style": "Regular",
                        },
                        "layout": {
                            "width": 6,
                            "height": 3,
                            "depth": body.get("depth", 1),
                            "voxelCount": 16,
                            "boxCount": len(MOCK.boxes),
                            "bounds": {
                                "min": body.get("origin", [0, 0, 0]),
                                "max": [
                                    body.get("origin", [0, 0, 0])[0] + 6,
                                    body.get("origin", [0, 0, 0])[1] + 3,
                                    body.get("origin", [0, 0, 0])[2]
                                    + body.get("depth", 1),
                                ],
                            },
                            "missingGlyphs": [],
                        },
                    }
                )
            if self.path == "/api/edit":
                rejected = False
                if body.get("op") in {"box", "erase"}:
                    group = body.get("group", "main")
                    if group in MOCK.protected_groups:
                        rejected = True
                if body.get("op") == "transform":
                    ids = set(body.get("ids", []))
                    if any(
                        box["id"] in ids
                        and box["group"] in MOCK.protected_groups
                        for box in MOCK.boxes
                    ):
                        rejected = True
                MOCK.edit_calls.append(
                    {
                        "op": body.get("op"),
                        "group": body.get("group"),
                        "ids": body.get("ids", []),
                        "rejected": rejected,
                    }
                )
                if rejected:
                    return self.send_json(
                        {
                            "ok": False,
                            "error": "PROTECTED_GROUP_CONFLICT：目标分组已受保护",
                        },
                        409,
                    )
                MOCK.revision += 1
                if body.get("op") == "undo":
                    MOCK.boxes = []
                elif body.get("op") == "clear":
                    MOCK.boxes = []
                elif body.get("op") == "box":
                    group = body.get("group", "main")
                    MOCK.boxes.append(
                        {
                            "id": 1000 + MOCK.revision,
                            "min": body.get("min", [0, 0, 0]),
                            "size": body.get("size", [1, 1, 1]),
                            "styleId": body.get("styleId", 1),
                            "tintId": body.get("tintId", 0),
                            "motionPhase": 0,
                            "group": group,
                        }
                    )
                return self.send_json({"ok": True, "revision": MOCK.revision})
            if self.path == "/api/harness/config":
                MOCK.state()["harness"]["settings"].update(body)
                return self.send_json(MOCK.state())
            if self.path == "/api/project":
                MOCK.last_project = body
                if (
                    body.get("baseRevision") is not None
                    and body["baseRevision"] != MOCK.revision
                ):
                    return self.send_json(
                        {"ok": False, "error": "REVISION_CONFLICT"}, 409
                    )
                if "protectedGroups" in body:
                    next_groups = list(dict.fromkeys(body["protectedGroups"]))
                    if next_groups != MOCK.protected_groups:
                        MOCK.protected_groups = next_groups
                        MOCK.revision += 1
                return self.send_json(MOCK.state())
            if self.path in {
                "/api/connect",
                "/api/publish",
                "/api/placement",
                "/api/capture/result",
            }:
                return self.send_json({"ok": True})
        self.send_json({"ok": False, "error": f"unhandled {self.path}"}, 404)

    def handle_message(self, body):
        request_id = body.get("requestId")
        if not request_id:
            return self.send_json({"ok": False, "error": "requestId required"}, 400)
        if request_id in MOCK.requests:
            return self.send_json(MOCK.requests[request_id])
        if MOCK.run and body.get("runId") and body["runId"] != MOCK.run["id"]:
            return self.send_json(
                {"ok": False, "error": "RUN_CONFLICT: stale runId"}, 409
            )
        prompt = str(body.get("prompt", ""))
        mode = body.get("mode")
        if mode not in {"steer", "interrupt", "queue"}:
            return self.send_json({"ok": False, "error": "invalid mode"}, 400)
        if not prompt.strip() and mode != "interrupt":
            return self.send_json({"ok": False, "error": "prompt required"}, 400)
        MOCK.message_calls += 1
        time.sleep(MOCK.message_delay)
        if not MOCK.run:
            MOCK.run = {
                "id": "run-1",
                "status": "running",
                "prompt": prompt,
                "steps": 0,
                "tokens": 0,
                "message": "我正在检查环境。",
                "error": "",
            }
            MOCK.phase = "running"
            MOCK.epoch = 1
        elif mode == "interrupt":
            MOCK.epoch += 1
            MOCK.phase = "paused" if not prompt else "running"
            MOCK.paused = not bool(prompt)
            MOCK.run["message"] = prompt if prompt else "灯塔主体已完成。"
        elif mode == "queue":
            MOCK.queue.append({"requestId": request_id, "mode": mode})
            MOCK.epoch += 1
        else:
            MOCK.epoch += 1
            MOCK.run["message"] += "\n收到插话，正在调整。"
        ack = {
            "ok": True,
            "accepted": True,
            "requestId": request_id,
            "runId": MOCK.run["id"],
            "epoch": MOCK.epoch,
            "phase": MOCK.phase,
            "queuePosition": len(MOCK.queue) if mode == "queue" else None,
        }
        MOCK.requests[request_id] = ack
        MOCK.event("harness.message.accepted", request_id, prompt=prompt, mode=mode)
        if mode == "steer":
            MOCK.event("harness.message.applied", request_id, prompt=prompt, mode=mode)
        return self.send_json(ack)


class UiBrowser(Browser):
    def css_click(self, selector):
        element = self.call(
            self.path + "/element",
            {"using": "css selector", "value": selector},
        )
        self.call(
            self.path
            + "/element/"
            + element["element-6066-11e4-a52e-4f735466cecf"]
            + "/click",
            {},
        )

    def set_value(self, identifier, value):
        self.js(
            """const e=document.getElementById(arguments[0]);e.value=arguments[1];
               e.dispatchEvent(new Event('input',{bubbles:true}));
               e.dispatchEvent(new Event('change',{bubbles:true}));""",
            identifier,
            value,
        )


def post_json(url, value):
    request = urllib.request.Request(
        url,
        data=json.dumps(value, ensure_ascii=False).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=20) as response:
        return json.load(response)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", help="run against an already-running real app")
    parser.add_argument("--label", default="mock")
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    server = None
    origin = args.url
    if not origin:
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{server.server_port}"
    browser = UiBrowser()
    checks = []
    last_browser_logs = []

    def check(name, passed):
        checks.append({"name": name, "pass": bool(passed)})
        if not passed:
            raise AssertionError(name)

    try:
        browser.call(browser.path + "/url", {"url": origin + "/"})
        browser.wait("return window.__morphforge?.ready")
        check(
            "four clear workbench tabs",
            browser.js(
                "return [...document.querySelectorAll('[data-tab]')].map(e=>e.textContent).join('|')==='AI创作|字体文字|VRC发布|上下文'"
            ),
        )
        check(
            "idle primary action is explicit",
            browser.js(
                "return document.getElementById('sendMessage').textContent==='开始创作'"
            ),
        )

        if server:
            browser.point([0.125, 0, 0.125])
            browser.wait(
                "return window.__morphforge.state.stats.boxCount===1"
            )
            browser.wait(
                "return !!document.querySelector('.group-protect[data-group=\"main\"]')"
            )
            browser.set_value("prompt", "保护冲突时这段输入必须保留")
            protection_revision = browser.js(
                "return window.__morphforge.state.revision"
            )
            browser.css_click('.group-protect[data-group="main"]')
            browser.wait(
                "return document.querySelector('.group-protect[data-group=\"main\"]').getAttribute('aria-pressed')==='true'"
            )
            check(
                "group protection persists independently of visibility",
                browser.js(
                    "const row=document.querySelector('.group-row[data-group=\"main\"]');return row.querySelector('.group-visibility').checked&&row.querySelector('.group-protect').getAttribute('aria-pressed')==='true'&&window.__morphforge.state.project.protectedGroups.includes('main')"
                ),
            )
            check(
                "group protection request carries the current baseRevision and creates a revision",
                MOCK.last_project.get("protectedGroups") == ["main"]
                and MOCK.last_project.get("baseRevision") == protection_revision
                and browser.js("return window.__morphforge.state.revision")
                == protection_revision + 1,
            )
            browser.screenshot(OUT / f"{args.label}-protected-group.png")
            browser.js(
                "const e=document.getElementById('prompt');e.value=arguments[0];e.dispatchEvent(new Event('input',{bubbles:true}))",
                "保护冲突时这段输入必须保留",
            )
            browser.point([0.125, 0.25, 0.125])
            browser.wait(
                "return document.getElementById('toast').textContent.includes('已锁定')"
            )
            check(
                "protected group edit is rejected with Chinese guidance and input preserved",
                browser.js(
                    "return window.__morphforge.state.stats.boxCount===1&&document.getElementById('prompt').value==='保护冲突时这段输入必须保留'&&document.getElementById('toast').textContent.includes('解除保护')"
                ),
            )
            browser.js(
                "document.getElementById('toast').textContent='';document.getElementById('toast').classList.remove('visible')"
            )
            browser.point([0.125, 0, 0.125], shift=True)
            browser.wait(
                "return document.getElementById('toast').textContent.includes('已锁定')"
            )
            check(
                "protected group erase is rejected by the UI contract",
                server
                and MOCK.edit_calls[-1]["op"] == "erase"
                and MOCK.edit_calls[-1]["rejected"]
                and browser.js("return window.__morphforge.state.stats.boxCount")
                == 1,
            )
            browser.css_click('.group-row[data-group="main"] > span')
            browser.set_value("coordX", "1")
            browser.js(
                "document.getElementById('toast').textContent='';document.getElementById('toast').classList.remove('visible')"
            )
            browser.click("moveSelection")
            browser.wait(
                "return document.getElementById('toast').textContent.includes('已锁定')"
            )
            check(
                "protected group transform is rejected and input remains",
                server
                and MOCK.edit_calls[-1]["op"] == "transform"
                and MOCK.edit_calls[-1]["rejected"]
                and browser.js("return window.__morphforge.state.stats.boxCount")
                == 1
                and browser.js("return document.getElementById('prompt').value")
                == "保护冲突时这段输入必须保留",
            )
            browser.css_click('.group-protect[data-group="main"]')
            browser.wait(
                "return document.querySelector('.group-protect[data-group=\"main\"]').getAttribute('aria-pressed')==='false'"
            )
            browser.point([0.125, 0.25, 0.125])
            browser.wait(
                "return window.__morphforge.state.stats.boxCount===2"
            )
            check(
                "unlocking protection permits the next edit",
                browser.js(
                    "return window.__morphforge.state.project.protectedGroups.length===0"
                ),
            )
            browser.click("clearScene")
            browser.wait(
                "return window.__morphforge.state.stats.boxCount===0"
            )
            browser.js("document.getElementById('prompt').value=''")

        browser.set_value("prompt", "启动灯塔")
        browser.js("document.getElementById('sendMessage').click()")
        browser.wait(
            "return window.__morphforge.harnessUi.pending().some(x=>x.status==='accepted'||x.status==='applied')"
        )
        check(
            "accepted start clears exactly the submitted prompt",
            browser.js("return document.getElementById('prompt').value===''"),
        )
        if server:
            post_json(
                origin + "/mock/advance",
                {
                    "runId": "run-1",
                    "status": "running",
                    "epoch": 1,
                    "phase": "running",
                    "prompt": "启动灯塔",
                    "message": "主体结构已就绪。",
                },
            )
        browser.wait(
            "return document.getElementById('runStatus').textContent.includes('运行中')"
        )

        browser.set_value("messageMode", "steer")
        browser.set_value("prompt", "增加一个门")
        browser.js(
            "const b=document.getElementById('sendMessage');b.click();b.click()"
        )
        browser.wait(
            "return window.__morphforge.harnessUi.pending().length>=2"
        )
        browser.wait(
            "return document.getElementById('sendMessage').disabled===false"
        )
        if server:
            before = MOCK.message_calls
        browser.set_value("prompt", "再加一扇窗")
        browser.js("document.getElementById('sendMessage').click()")
        browser.wait(
            "return window.__morphforge.harnessUi.pending().length>=3"
        )
        browser.wait(
            "return document.getElementById('sendMessage').disabled===false"
        )
        check(
            "double click and rapid interjections do not duplicate requestIds",
            browser.js(
                "const ids=window.__morphforge.harnessUi.pending().map(x=>x.requestId);return new Set(ids).size===ids.length"
            ),
        )
        if server:
            check("double click produced one request", MOCK.message_calls == before + 1)

        browser.set_value("messageMode", "interrupt")
        browser.wait(
            "return document.getElementById('sendMessage').textContent==='打断并继续'"
        )
        browser.set_value("prompt", "改成海边的灯塔")
        browser.js("document.getElementById('sendMessage').click()")
        browser.wait(
            "return document.querySelectorAll('.chat-entry.interrupted').length===1"
        )
        check(
            "interrupted assistant fragment remains labelled",
            browser.js(
                "const e=document.querySelector('.chat-entry.interrupted');return !!e&&!!e.querySelector('.entry-note')&&e.querySelector('.entry-note').textContent.trim().length>0"
            ),
        )

        browser.click("pauseAI")
        browser.wait(
            "return document.getElementById('runStatus').textContent.includes('已暂停')"
        )
        browser.wait(
            "return window.__morphforge.harnessUi.pending().some(x=>x.mode==='interrupt'&&!x.prompt&&x.status==='accepted')"
        )
        check(
            "pause uses the empty interrupt and is visible in the conversation",
            browser.js(
                "return window.__morphforge.harnessUi.chat().some(x=>x.text.includes('暂停并取消当前请求'))&&!document.getElementById('interruptAI')"
            ),
        )
        browser.click("resumeAI")
        browser.wait(
            "return !document.getElementById('runStatus').textContent.includes('已暂停')"
        )
        check("pause and resume expose distinct states", True)
        check(
            "pause exposes fast cancel semantics without a duplicate interrupt button",
            browser.js(
                "return document.getElementById('pauseAI').title.includes('取消')&&!document.getElementById('interruptAI')"
            ),
        )

        prior_assistant = browser.js(
            "return window.__morphforge.harnessUi.chat().filter(x=>x.role==='assistant').map(x=>({id:x.id,text:x.text}))"
        )
        final_summary = "作品完成：灯塔已建成，原有建筑保持不变。"
        if server:
            for _ in range(2):
                post_json(
                    origin + "/mock/advance",
                    {
                        "runId": "run-1",
                        "status": "completed",
                        "epoch": 2,
                        "phase": "completed",
                        "prompt": "启动灯塔",
                        "message": final_summary,
                        "emitFinished": True,
                    },
                )
        browser.wait(
            f"return window.__morphforge.harnessUi.chat().filter(x=>x.role==='assistant'&&x.text==={json.dumps(final_summary)}).length===1"
        )
        check(
            "completed Run summary is shown once from harness.finished",
            browser.js(
                """const final=window.__morphforge.harnessUi.chat().filter(x=>x.role==='assistant'&&x.text===arguments[0]);
                   return final.length===1&&final[0].final===true&&final[0].note==='完成总结'""",
                final_summary,
            ),
        )
        check(
            "completed summary preserves prior assistant fragments",
            browser.js(
                """const current=new Map(window.__morphforge.harnessUi.chat().map(x=>[x.id,x.text]));
                   return arguments[0].every(x=>current.get(x.id)===x.text)""",
                prior_assistant,
            ),
        )
        time.sleep(0.8)
        check(
            "repeated completion state and event do not duplicate summary",
            browser.js(
                "return window.__morphforge.harnessUi.chat().filter(x=>x.role==='assistant'&&x.text===arguments[0]).length===1",
                final_summary,
            ),
        )
        browser.screenshot(OUT / f"{args.label}-complete-summary.png")

        browser.set_value("prompt", "这段输入要在刷新后保留")
        browser.call(browser.path + "/url", {"url": origin + "/"})
        browser.wait("return window.__morphforge?.ready")
        check(
            "draft survives browser refresh",
            browser.js(
                "return document.getElementById('prompt').value==='这段输入要在刷新后保留'"
            ),
        )
        check(
            "chat and pending state do not duplicate after refresh",
            browser.js(
                "const ids=[...document.querySelectorAll('[data-chat-id]')].map(e=>e.dataset.chatId);return ids.length===new Set(ids).size"
            ),
        )
        browser.wait(
            f"return window.__morphforge.harnessUi.chat().filter(x=>x.role==='assistant'&&x.text==={json.dumps(final_summary)}).length===1"
        )
        check(
            "completed summary is not duplicated after refresh",
            browser.js(
                "return window.__morphforge.harnessUi.chat().filter(x=>x.role==='assistant'&&x.text===arguments[0]).length===1",
                final_summary,
            ),
        )

        browser.css_click('[data-tab="text"]')
        browser.wait("return !document.getElementById('textFont').disabled")
        browser.set_value("textContent", "中文\nAB")
        browser.set_value("textSize", "8")
        revision_before = browser.js("return window.__morphforge.state.revision")
        boxes_before = browser.js(
            "return window.__morphforge.renderer.boxes.length"
        )
        browser.click("textPreview")
        browser.wait("return !!window.__morphforge.renderer.textPreviewMesh")
        check(
            "text preview is an independent translucent overlay",
            browser.js(
                "const m=window.__morphforge.renderer.textPreviewMesh;return m.material.transparent&&m.material.opacity<1&&window.__morphforge.renderer.boxes.length===arguments[0]",
                boxes_before,
            ),
        )
        check(
            "text preview does not change the draft revision",
            browser.js("return window.__morphforge.state.revision") == revision_before,
        )
        check(
            "apply is enabled only after a current preview",
            browser.js("return !document.getElementById('textApply').disabled"),
        )

        browser.set_value("textContent", "中文\nABC")
        browser.wait(
            "return document.getElementById('textApply').disabled===true"
        )
        browser.click("textPreview")
        browser.wait(
            "return document.getElementById('textApply').disabled===false"
        )
        browser.click("textApply")
        browser.wait(
            f"return window.__morphforge.state.revision>{revision_before}"
        )
        check(
            "text apply clears preview and creates one revision",
            browser.js(
                "return window.__morphforge.renderer.textPreviewMesh===null&&window.__morphforge.state.revision>arguments[0]",
                revision_before,
            ),
        )
        browser.click("undo")
        browser.wait(
            f"return window.__morphforge.state.stats.boxCount==={boxes_before}"
        )
        check("text apply is undoable through the shared edit API", True)

        if server:
            check(
                "apply carries preview requestId and server fingerprint",
                len(MOCK.last_apply.get("previewFingerprint", "")) == 64
                and bool(MOCK.last_apply.get("previewRequestId")),
            )
            MOCK.preview_delay = 0.45
            before_preview_calls = MOCK.preview_calls
            browser.set_value("textGroup", "old-preview")
            browser.set_value("textContent", "旧结果")
            browser.click("textPreview")
            time.sleep(0.05)
            browser.set_value("textGroup", "new-preview")
            browser.set_value("textContent", "新结果")
            browser.wait(
                "return window.__morphforge.text.request()?.group==='new-preview'",
                5,
            )
            time.sleep(0.55)
            check(
                "late preview response cannot replace the latest preview",
                browser.js(
                    "return window.__morphforge.text.request()?.group==='new-preview'&&!document.getElementById('textApply').disabled"
                ),
            )
            check(
                "debounced race sent both previews and used only the latest",
                MOCK.preview_calls >= before_preview_calls + 2,
            )
            MOCK.preview_delay = 0.0

        browser.set_value("textContent", "缺字 😀")
        browser.click("textPreview")
        browser.wait(
            "return document.getElementById('textMetrics').textContent.includes('缺字')&&(document.getElementById('textMetrics').textContent.includes('😀')||document.getElementById('textMetrics').textContent.includes('U+1F600'))"
        )
        check(
            "missing glyph error is listed and apply stays disabled",
            browser.js(
                "return document.getElementById('textApply').disabled&&document.getElementById('textMetrics').textContent.includes('缺字')"
            ),
        )

        logs = browser.call(browser.path + "/log", {"type": "browser"})
        last_browser_logs = logs
        severe = [
            entry
            for entry in logs
            if entry["level"] == "SEVERE"
            and "favicon.ico" not in entry["message"]
            and "PROTECTED_GROUP_CONFLICT" not in entry["message"]
            and not (
                any(
                    str(status) in entry["message"]
                    for status in (400, 409)
                )
                and "Failed to load resource" in entry["message"]
            )
        ]
        check("no browser JavaScript or WebGL errors", not severe)
        browser.screenshot(OUT / f"{args.label}-workbench.png")
        report = {
            "success": True,
            "checks": checks,
            "scope": (
                "Real Chromium UI flow against a contract-compatible mock API"
                if server
                else "Real Chromium UI flow against the running MorphForge app"
            ),
            "mock": {
                "messageCalls": MOCK.message_calls,
                "previewCalls": MOCK.preview_calls,
                "applyCalls": MOCK.apply_calls,
            }
            if server
            else None,
            "browserLogs": logs,
        }
    except Exception as exc:
        browser.screenshot(OUT / f"{args.label}-failure.png")
        captured_logs = browser.call(browser.path + "/log", {"type": "browser"})
        report = {
            "success": False,
            "checks": checks,
            "error": str(exc),
            "browserLogs": captured_logs or last_browser_logs,
        }
        raise
    finally:
        (OUT / f"{args.label}-validation.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2)
        )
        browser.close()
        if server:
            server.shutdown()
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
