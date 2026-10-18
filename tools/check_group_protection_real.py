#!/usr/bin/env python3
"""Exercise protected-group hard constraints through the real UI and Rust API."""

from __future__ import annotations

import argparse
import base64
import json
import mimetypes
import threading
import time
import urllib.error
import urllib.request
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from check_editor_browser import Browser

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/ui-v2"
WEB = ROOT / "web"


def make_proxy_handler(backend):
    class FrontendProxy(BaseHTTPRequestHandler):
        server_version = "MorphForgeFrontendProxy/1"

        def log_message(self, *_args):
            return

        def _backend_url(self):
            return backend + self.path

        def _proxy(self):
            length = int(self.headers.get("Content-Length", "0"))
            body = self.rfile.read(length) if length else None
            headers = {}
            content_type = self.headers.get("Content-Type")
            if content_type:
                headers["Content-Type"] = content_type
            accept = self.headers.get("Accept")
            if accept:
                headers["Accept"] = accept
            request = urllib.request.Request(
                self._backend_url(),
                data=body,
                headers=headers,
                method=self.command,
            )
            try:
                with urllib.request.urlopen(request, timeout=180) as response:
                    payload = response.read()
                    status = response.status
                    response_type = response.headers.get(
                        "Content-Type", "application/octet-stream"
                    )
            except urllib.error.HTTPError as error:
                payload = error.read()
                status = error.code
                response_type = error.headers.get(
                    "Content-Type", "application/octet-stream"
                )
            self.send_response(status)
            self.send_header("Content-Type", response_type)
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            self.wfile.write(payload)

        def do_GET(self):
            if self.path.startswith(("/api/", "/artifacts/")):
                return self._proxy()
            path = self.path.split("?", 1)[0].lstrip("/") or "studio.html"
            target = (WEB / path).resolve()
            if target != WEB and WEB not in target.parents:
                self.send_error(404)
                return
            if not target.is_file():
                self.send_error(404)
                return
            payload = target.read_bytes()
            self.send_response(200)
            self.send_header(
                "Content-Type",
                mimetypes.guess_type(target.name)[0]
                or "application/octet-stream",
            )
            self.send_header("Content-Length", str(len(payload)))
            self.send_header("Cache-Control", "no-cache")
            self.end_headers()
            self.wfile.write(payload)

        def do_POST(self):
            if self.path.startswith("/api/"):
                return self._proxy()
            self.send_error(404)

    return FrontendProxy


def request_json(origin, method, path, body=None, expected=(200,)):
    data = None if body is None else json.dumps(body, ensure_ascii=False).encode()
    request = urllib.request.Request(
        origin + path,
        data=data,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=90) as response:
            status = response.status
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        status = error.code
        payload = json.loads(error.read() or b"{}")
    if status not in expected:
        raise AssertionError(f"{method} {path}: HTTP {status}: {payload}")
    return status, payload


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


def state_revision(origin):
    return int(request_json(origin, "GET", "/api/state")[1]["revision"])


def clear_scene(origin):
    state = request_json(origin, "GET", "/api/state")[1]
    if state["project"].get("protectedGroups"):
        request_json(
            origin,
            "POST",
            "/api/project",
            {
                "protectedGroups": [],
                "baseRevision": state["revision"],
            },
        )
        state = request_json(origin, "GET", "/api/state")[1]
    revision = int(state["revision"])
    request_json(
        origin,
        "POST",
        "/api/edit",
        {
            "op": "clear",
            "baseRevision": revision,
            "requestId": "group-protection-clear-" + uuid.uuid4().hex,
        },
    )
    state = request_json(origin, "GET", "/api/state")[1]
    if state["stats"]["boxCount"] != 0:
        raise AssertionError("clear did not produce an empty scene")
    return state


def latest_seq(origin):
    events = request_json(origin, "GET", "/api/events?after=0")[1]["events"]
    return int(events[-1]["seq"]) if events else 0


def clear_toast(browser):
    browser.js(
        "document.getElementById('toast').textContent='';"
        "document.getElementById('toast').classList.remove('visible')"
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:8781")
    parser.add_argument("--label", default="group-protection-real")
    parser.add_argument(
        "--serve-local-frontend",
        action="store_true",
        help="Serve the current web/ source on a same-origin test proxy.",
    )
    args = parser.parse_args()
    origin = args.url.rstrip("/")
    browser_origin = origin
    proxy = None
    OUT.mkdir(parents=True, exist_ok=True)
    checks = []

    def check(name, passed, detail=None):
        entry = {"name": name, "pass": bool(passed)}
        if detail is not None:
            entry["detail"] = detail
        checks.append(entry)
        if not passed:
            raise AssertionError(name)

    tools_version, tools = request_json(
        origin, "GET", "/api/harness/tools?language=zh-CN"
    )
    state = request_json(origin, "GET", "/api/state")[1]
    initial_seq = latest_seq(origin)
    check(
        "R2 tool contract is loaded",
        tools_version == 200 and tools.get("version") == "morphforge-tools/2.2",
        tools.get("version"),
    )
    check(
        "provider is local mock and OSC is disabled before test",
        state["harness"]["provider"]["baseUrl"] == "http://127.0.0.1:8799"
        and state["connection"]["enabled"] is False,
    )
    baseline_state = clear_scene(origin)
    check(
        "isolated test starts with an empty scene and protection list",
        baseline_state["project"].get("protectedGroups") == []
        and baseline_state["stats"]["boxCount"] == 0,
    )

    if args.serve_local_frontend:
        proxy = ThreadingHTTPServer(
            ("127.0.0.1", 0), make_proxy_handler(origin)
        )
        threading.Thread(target=proxy.serve_forever, daemon=True).start()
        browser_origin = f"http://127.0.0.1:{proxy.server_port}"

    browser = UiBrowser()
    try:
        browser.call(browser.path + "/url", {"url": browser_origin + "/"})
        browser.wait("return window.__morphforge?.ready", 30)
        browser.set_value("groupName", "protected-demo")
        browser.point([0.125, 0, 0.125])
        browser.wait(
            "return window.__morphforge.state.stats.boxCount===1"
        )
        browser.wait(
            "return !!document.querySelector('.group-protect[data-group=\"protected-demo\"]')"
        )
        box_before = request_json(origin, "GET", "/api/scene")[1]["scene"]["boxes"]
        revision_before_protection = browser.js(
            "return window.__morphforge.state.revision"
        )
        browser.css_click(
            '.group-protect[data-group="protected-demo"]'
        )
        browser.wait(
            "return document.querySelector('.group-protect[data-group=\"protected-demo\"]')?.getAttribute('aria-pressed')==='true'"
        )
        state_locked = request_json(origin, "GET", "/api/state")[1]
        revision_locked = state_locked["revision"]
        check(
            "locking persists protectedGroups and creates a revision",
            state_locked["project"]["protectedGroups"] == ["protected-demo"]
            and revision_locked == revision_before_protection + 1,
            {
                "beforeRevision": revision_before_protection,
                "lockedRevision": revision_locked,
            },
        )
        check(
            "OSC remains disabled after locking",
            state_locked["connection"]["enabled"] is False,
        )

        direct_status, direct_draw = request_json(
            origin,
            "POST",
            "/api/edit",
            {
                "op": "box",
                "min": [0, 1, 0],
                "size": [1, 1, 1],
                "styleId": box_before[0]["styleId"],
                "tintId": 0,
                "group": "protected-demo",
                "baseRevision": revision_locked,
                "requestId": "group-protection-draw-" + uuid.uuid4().hex,
            },
            expected=(409,),
        )
        check(
            "backend returns 409 PROTECTED_GROUP_CONFLICT for draw",
            direct_status == 409
            and "PROTECTED_GROUP_CONFLICT" in str(direct_draw.get("error")),
            {"status": direct_status, "error": direct_draw.get("error")},
        )

        direct_status, direct_erase = request_json(
            origin,
            "POST",
            "/api/edit",
            {
                "op": "erase",
                "min": box_before[0]["min"],
                "size": box_before[0]["size"],
                "baseRevision": revision_locked,
                "requestId": "group-protection-erase-" + uuid.uuid4().hex,
            },
            expected=(409,),
        )
        check(
            "backend returns 409 PROTECTED_GROUP_CONFLICT for erase",
            direct_status == 409
            and "PROTECTED_GROUP_CONFLICT" in str(direct_erase.get("error")),
            {"status": direct_status, "error": direct_erase.get("error")},
        )

        direct_status, direct_transform = request_json(
            origin,
            "POST",
            "/api/edit",
            {
                "op": "transform",
                "ids": [box_before[0]["id"]],
                "offset": [1, 0, 0],
                "turns": 0,
                "mirror": None,
                "copy": False,
                "baseRevision": revision_locked,
                "requestId": "group-protection-transform-" + uuid.uuid4().hex,
            },
            expected=(409,),
        )
        check(
            "backend returns 409 PROTECTED_GROUP_CONFLICT for transform",
            direct_status == 409
            and "PROTECTED_GROUP_CONFLICT" in str(direct_transform.get("error")),
            {
                "status": direct_status,
                "error": direct_transform.get("error"),
            },
        )
        direct_scene = request_json(origin, "GET", "/api/scene")[1]["scene"]["boxes"]
        check(
            "direct backend conflicts leave geometry and revision unchanged",
            direct_scene == box_before
            and request_json(origin, "GET", "/api/state")[1]["revision"]
            == revision_locked,
        )

        future_group = "future-protected-empty"
        before_future_state = request_json(origin, "GET", "/api/state")[1]
        future_groups = list(before_future_state["project"]["protectedGroups"])
        future_groups.append(future_group)
        _, future_locked_state = request_json(
            origin,
            "POST",
            "/api/project",
            {
                "protectedGroups": future_groups,
                "baseRevision": before_future_state["revision"],
            },
        )
        revision_future_locked = int(future_locked_state["revision"])
        check(
            "backend accepts a future empty protected group",
            future_group in future_locked_state["project"]["protectedGroups"]
            and future_locked_state["stats"]["groups"].get(future_group, 0) == 0,
            {"revision": revision_future_locked},
        )
        direct_status, future_draw = request_json(
            origin,
            "POST",
            "/api/edit",
            {
                "op": "box",
                "min": [3, 0, 0],
                "size": [1, 1, 1],
                "styleId": box_before[0]["styleId"],
                "tintId": 0,
                "group": future_group,
                "baseRevision": revision_future_locked,
                "requestId": "group-protection-future-" + uuid.uuid4().hex,
            },
            expected=(409,),
        )
        check(
            "backend rejects the first draw into a future protected group",
            direct_status == 409
            and "PROTECTED_GROUP_CONFLICT"
            in str(future_draw.get("error")),
            {"status": direct_status, "error": future_draw.get("error")},
        )
        browser.call(browser.path + "/url", {"url": browser_origin + "/"})
        browser.wait("return window.__morphforge?.ready", 30)
        browser.wait(
            """return [...document.querySelectorAll('.group-row')].some(row=>row.dataset.group==='future-protected-empty'&&row.classList.contains('empty'))"""
        )
        future_row = browser.js(
            """const row=document.querySelector('.group-row[data-group="future-protected-empty"]');
               return !!row&&row.querySelector('small').textContent==='0'&&row.querySelector('.group-protect').getAttribute('aria-pressed')==='true'"""
        )
        check(
            "future empty group is visible as zero and unlockable",
            future_row,
        )
        browser.css_click(
            '.group-protect[data-group="future-protected-empty"]'
        )
        browser.wait(
            """return !window.__morphforge.state.project.protectedGroups.includes('future-protected-empty')&&document.getElementById('toast').textContent.includes('解除分组')"""
        )
        state_future_unlocked = request_json(origin, "GET", "/api/state")[1]
        check(
            "future group can be unlocked from the UI",
            future_group
            not in state_future_unlocked["project"]["protectedGroups"],
            {"revision": state_future_unlocked["revision"]},
        )
        revision_future_unlocked = int(state_future_unlocked["revision"])
        direct_status, future_unlocked_draw = request_json(
            origin,
            "POST",
            "/api/edit",
            {
                "op": "box",
                "min": [3, 0, 0],
                "size": [1, 1, 1],
                "styleId": box_before[0]["styleId"],
                "tintId": 0,
                "group": future_group,
                "baseRevision": revision_future_unlocked,
                "requestId": "group-protection-future-open-"
                + uuid.uuid4().hex,
            },
        )
        state_future_open = request_json(origin, "GET", "/api/state")[1]
        check(
            "a formerly future group accepts drawing after unlock",
            direct_status == 200
            and state_future_open["stats"]["groups"].get(future_group, 0) >= 1,
            {"revision": state_future_open["revision"]},
        )
        browser.screenshot(OUT / f"{args.label}-locked.png")

        # A fresh page load proves the setting came from state, not local UI.
        browser.call(browser.path + "/url", {"url": browser_origin + "/"})
        browser.wait("return window.__morphforge?.ready", 30)
        browser.set_value("groupName", "protected-demo")
        browser.wait(
            "return document.querySelector('.group-protect[data-group=\"protected-demo\"]')?.getAttribute('aria-pressed')==='true'"
        )
        draft = "保护冲突时这段输入必须保留"
        browser.set_value("prompt", draft)
        revision_before_ui_conflicts = request_json(
            origin, "GET", "/api/state"
        )[1]["revision"]
        scene_before_ui_conflicts = request_json(origin, "GET", "/api/scene")[1][
            "scene"
        ]["boxes"]

        clear_toast(browser)
        browser.point([0.125, 0.25, 0.125])
        browser.wait(
            "return document.getElementById('toast').textContent.includes('目标分组已锁定')"
        )
        state_draw = request_json(origin, "GET", "/api/state")[1]
        scene_draw = request_json(origin, "GET", "/api/scene")[1]["scene"]["boxes"]
        check(
            "real draw conflict shows Chinese guidance and preserves draft",
            browser.js("return document.getElementById('prompt').value")
            == draft
            and state_draw["revision"] == revision_before_ui_conflicts
            and scene_draw == scene_before_ui_conflicts,
            {"revision": state_draw["revision"], "boxes": len(scene_draw)},
        )

        clear_toast(browser)
        browser.point([0.125, 0, 0.125], shift=True)
        browser.wait(
            "return document.getElementById('toast').textContent.includes('目标分组已锁定')"
        )
        state_erase = request_json(origin, "GET", "/api/state")[1]
        scene_erase = request_json(origin, "GET", "/api/scene")[1]["scene"]["boxes"]
        check(
            "real erase conflict leaves geometry unchanged",
            state_erase["revision"] == revision_before_ui_conflicts
            and scene_erase == scene_before_ui_conflicts,
            {"revision": state_erase["revision"], "boxes": len(scene_erase)},
        )

        browser.css_click(
            '.group-row[data-group="protected-demo"] > span'
        )
        browser.set_value("coordX", "1")
        clear_toast(browser)
        browser.click("moveSelection")
        browser.wait(
            "return document.getElementById('toast').textContent.includes('目标分组已锁定')"
        )
        state_transform = request_json(origin, "GET", "/api/state")[1]
        scene_transform = request_json(origin, "GET", "/api/scene")[1][
            "scene"
        ]["boxes"]
        check(
            "real transform conflict leaves geometry unchanged",
            state_transform["revision"] == revision_before_ui_conflicts
            and scene_transform == scene_before_ui_conflicts,
            {
                "revision": state_transform["revision"],
                "boxes": len(scene_transform),
            },
        )
        check(
            "all conflict paths preserve the unsent prompt",
            browser.js("return document.getElementById('prompt').value") == draft,
        )
        check(
            "OSC remains disabled through all conflicts",
            state_transform["connection"]["enabled"] is False,
        )

        browser.css_click(
            '.group-protect[data-group="protected-demo"]'
        )
        browser.wait(
            "return document.querySelector('.group-protect[data-group=\"protected-demo\"]')?.getAttribute('aria-pressed')==='false'"
        )
        state_unlocked = request_json(origin, "GET", "/api/state")[1]
        check(
            "unlock persists an empty protectedGroups list",
            state_unlocked["project"]["protectedGroups"] == [],
            {"revision": state_unlocked["revision"]},
        )

        browser.point([0.125, 0.25, 0.125])
        browser.wait(
            "return window.__morphforge.state.stats.boxCount===2"
        )
        state_after_edit = request_json(origin, "GET", "/api/state")[1]
        scene_after_edit = request_json(origin, "GET", "/api/scene")[1]["scene"][
            "boxes"
        ]
        check(
            "after unlocking the next real draw succeeds",
            state_after_edit["revision"] > state_unlocked["revision"]
            and scene_after_edit != scene_before_ui_conflicts
            and state_after_edit["connection"]["enabled"] is False,
            {"revision": state_after_edit["revision"]},
        )

        direct_status, direct_unlocked_draw = request_json(
            origin,
            "POST",
            "/api/edit",
            {
                "op": "box",
                "min": [0, 2, 0],
                "size": [1, 1, 1],
                "styleId": box_before[0]["styleId"],
                "tintId": 0,
                "group": "protected-demo",
                "baseRevision": state_after_edit["revision"],
                "requestId": "group-protection-unlocked-" + uuid.uuid4().hex,
            },
        )
        state_after_direct_unlock_edit = request_json(
            origin, "GET", "/api/state"
        )[1]
        check(
            "backend permits a direct edit after unlocking",
            direct_status == 200
            and state_after_direct_unlock_edit["revision"]
            > state_after_edit["revision"]
            and state_after_direct_unlock_edit["connection"]["enabled"] is False,
            {
                "revision": state_after_direct_unlock_edit["revision"],
                "boxCount": state_after_direct_unlock_edit["stats"]["boxCount"],
            },
        )
        browser.screenshot(OUT / f"{args.label}-unlocked.png")

        new_events = request_json(
            origin, "GET", f"/api/events?after={initial_seq}"
        )[1]["events"]
        model_events = [
            event
            for event in new_events
            if event["kind"].startswith("provider.")
            or event["kind"].startswith("harness.message.")
        ]
        check(
            "no provider or Harness message requests were sent",
            not model_events,
            [event["kind"] for event in model_events],
        )

        clear_toast(browser)
        browser.click("clearScene")
        browser.wait(
            "return window.__morphforge.state.stats.boxCount===0"
        )
        final_state = request_json(origin, "GET", "/api/state")[1]
        check(
            "protection test cleans its geometry and leaves OSC disabled",
            final_state["stats"]["boxCount"] == 0
            and final_state["connection"]["enabled"] is False,
            {
                "revision": final_state["revision"],
                "protectedGroups": final_state["project"].get(
                    "protectedGroups", []
                ),
            },
        )

        logs = browser.call(browser.path + "/log", {"type": "browser"})
        severe = [
            entry
            for entry in logs
            if entry["level"] == "SEVERE"
            and "favicon.ico" not in entry["message"]
            and "PROTECTED_GROUP_CONFLICT" not in entry["message"]
            and not (
                "409" in entry["message"]
                and "Failed to load resource" in entry["message"]
            )
        ]
        check(
            "no unexpected JavaScript or WebGL errors",
            not severe,
            severe,
        )
        report = {
            "schema": "MorphForgeGroupProtectionReal/1",
            "success": True,
            "scope": (
                "Real Rust R2b backend with current web/ served by a same-origin test proxy, real Chromium UI, mock provider only"
                if args.serve_local_frontend
                else "Frozen R2b backend-embedded UI, real Chromium, mock provider only"
            ),
            "url": origin,
            "browserOrigin": browser_origin,
            "localFrontendProxy": bool(args.serve_local_frontend),
            "toolsVersion": tools.get("version"),
            "revisions": {
                "beforeProtection": revision_before_protection,
                "locked": revision_locked,
                "afterConflicts": state_transform["revision"],
                "unlocked": state_unlocked["revision"],
                "afterEdit": state_after_edit["revision"],
                "afterDirectUnlockedEdit": state_after_direct_unlock_edit[
                    "revision"
                ],
                "final": final_state["revision"],
            },
            "boxCounts": {
                "beforeProtection": len(box_before),
                "afterDrawConflict": len(scene_draw),
                "afterEraseConflict": len(scene_erase),
                "afterTransformConflict": len(scene_transform),
                "afterUnlockedEdit": state_after_edit["stats"]["boxCount"],
                "afterDirectUnlockedEdit": state_after_direct_unlock_edit[
                    "stats"
                ]["boxCount"],
                "final": final_state["stats"]["boxCount"],
            },
            "protectedGroups": {
                "locked": state_locked["project"]["protectedGroups"],
                "unlocked": state_unlocked["project"]["protectedGroups"],
            },
            "oscEnabledAtEveryCheckedStage": False,
            "modelEvents": model_events,
            "checks": checks,
            "screenshots": [
                f"docs/ui-v2/{args.label}-locked.png",
                f"docs/ui-v2/{args.label}-unlocked.png",
            ],
            "browserLogs": logs,
        }
    except Exception as error:
        browser.screenshot(OUT / f"{args.label}-failure.png")
        report = {
            "schema": "MorphForgeGroupProtectionReal/1",
            "success": False,
            "error": str(error),
            "checks": checks,
            "browserLogs": browser.call(browser.path + "/log", {"type": "browser"}),
        }
        raise
    finally:
        (OUT / f"{args.label}-validation.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n"
        )
        browser.close()
        if proxy:
            proxy.shutdown()
    print(
        json.dumps(
            {
                "success": True,
                "checks": len(checks),
                "report": f"docs/ui-v2/{args.label}-validation.json",
            },
            ensure_ascii=False,
        )
    )


if __name__ == "__main__":
    main()
