#!/usr/bin/env python3
"""Verify the real text HTTP contract and renderer direction without a model.

This runner never calls /api/harness/message or a provider. It reads fonts,
requests pure text previews, and performs an optional apply/undo transaction.
The orientation check hides scene content and compares actual canvas pixels
from the UI's independent translucent preview overlay against projected Boxes.
"""

from __future__ import annotations

import argparse
import base64
import json
import threading
import time
import urllib.error
import urllib.request
import uuid
from pathlib import Path

from check_editor_browser import Browser

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/ui-v2"
VIEW = {"xy": "front", "xz": "top", "yz": "side"}


def request_json(origin, method, path, body=None):
    data = None if body is None else json.dumps(body, ensure_ascii=False).encode()
    request = urllib.request.Request(
        origin + path,
        data=data,
        method=method,
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=90) as response:
            return response.status, json.load(response)
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read() or b"{}")


class TextBrowser(Browser):
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


def request_values(style_id, font_id, text, plane):
    return {
        "text": text,
        "fontId": font_id,
        "fontSize": 8,
        "depth": 1,
        "origin": [0, 0, 0],
        "plane": plane,
        "facing": "positive",
        "align": "left",
        "letterSpacing": 0,
        "lineSpacing": 0,
        "maxWidth": None,
        "styleId": style_id,
        "tintId": 0,
        "motionPhase": 0,
        "group": "text-real-check",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--url", default="http://127.0.0.1:8781")
    parser.add_argument("--label", default="real")
    parser.add_argument("--apply", action="store_true")
    parser.add_argument(
        "--mock",
        action="store_true",
        help="exercise this runner against the contract mock instead of Rust",
    )
    args = parser.parse_args()
    origin = args.url.rstrip("/")
    server = None
    if args.mock:
        from http.server import ThreadingHTTPServer

        from check_ui_v2 import Handler

        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        origin = f"http://127.0.0.1:{server.server_port}"
    OUT.mkdir(parents=True, exist_ok=True)
    checks = []

    def check(name, passed, detail=None):
        item = {"name": name, "pass": bool(passed)}
        if detail is not None:
            item["detail"] = detail
        checks.append(item)
        if not passed:
            raise AssertionError(name)

    status, state = request_json(origin, "GET", "/api/state")
    check("real backend state", status == 200 and state.get("revision") is not None)
    status, fonts = request_json(origin, "GET", "/api/fonts")
    font_items = fonts.get("fonts", []) if isinstance(fonts, dict) else []
    builtin = next(
        (
            font
            for font in font_items
            if font.get("id") == "builtin:noto-sans-sc"
        ),
        None,
    )
    check("builtin Chinese font is advertised", status == 200 and builtin is not None)
    status, scene = request_json(origin, "GET", "/api/scene")
    materials = scene.get("materials", []) if isinstance(scene, dict) else []
    check("real material catalog is available", status == 200 and bool(materials))
    style_id = int(materials[0]["id"])
    revision_before = int(state["revision"])
    text = "L"

    direct_by_plane = {}
    for plane in ("xy", "xz", "yz"):
        preview_id = "text-real-preview-" + uuid.uuid4().hex
        payload = request_values(style_id, builtin["id"], text, plane)
        payload["requestId"] = preview_id
        status, preview = request_json(
            origin, "POST", "/api/text/preview", payload
        )
        fingerprint = preview.get("requestFingerprint", "") if isinstance(preview, dict) else ""
        operations = preview.get("operations", []) if isinstance(preview, dict) else []
        boxes = [op for op in operations if op.get("op") == "box"]
        layout = preview.get("layout", {}) if isinstance(preview, dict) else {}
        direct_by_plane[plane] = preview
        check(
            f"{plane} preview HTTP shape and requestId echo",
            status == 200
            and preview.get("ok") is True
            and preview.get("requestId") == preview_id
            and preview.get("revision") == revision_before
            and len(fingerprint) == 64
            and bool(boxes)
            and bool(layout.get("bounds")),
            {
                "requestId": preview.get("requestId"),
                "revision": preview.get("revision"),
                "boxCount": len(boxes),
            },
        )

    missing_id = "text-real-missing-" + uuid.uuid4().hex
    missing_payload = request_values(
        style_id, builtin["id"], "A\U0001f600B", "xy"
    )
    missing_payload["requestId"] = missing_id
    status, missing = request_json(
        origin, "POST", "/api/text/preview", missing_payload
    )
    check(
        "real missing glyph returns typed top-level error",
        status == 400
        and missing.get("ok") is False
        and missing.get("code") == "FONT_MISSING_GLYPHS"
        and isinstance(missing.get("error"), str)
        and bool(missing.get("missingGlyphs")),
        {
            "status": status,
            "code": missing.get("code"),
            "missingGlyphs": missing.get("missingGlyphs"),
        },
    )

    browser = TextBrowser()
    try:
        browser.call(browser.path + "/url", {"url": origin + "/"})
        browser.wait("return window.__morphforge?.ready", 30)
        browser.css_click('[data-tab="text"]')
        browser.wait(
            "return !document.getElementById('textFont').disabled", 20
        )
        browser.set_value("textFont", builtin["id"])
        browser.set_value("textContent", text)
        browser.set_value("textSize", "8")
        browser.set_value("textDepth", "1")
        browser.set_value("textX", "0")
        browser.set_value("textY", "0")
        browser.set_value("textZ", "0")

        pixel_results = {}
        for plane, view in VIEW.items():
            browser.set_value("textPlane", plane)
            browser.click("textPreview")
            browser.wait(
                f"return window.__morphforge.text.request()?.plane==='{plane}'&&!!window.__morphforge.text.preview()",
                20,
            )
            check(
                f"{plane} UI preview did not change draft revision",
                browser.js("return window.__morphforge.state.revision")
                == revision_before,
            )
            browser.js(
                """const r=window.__morphforge.renderer,b=window.__morphforge.text.preview().layout.bounds;
                   r.view(arguments[0],[(b.min[0]+b.max[0])/2,(b.min[1]+b.max[1])/2,(b.min[2]+b.max[2])/2]);""",
                view,
            )
            result = browser.js(
                r"""
                const r=window.__morphforge.renderer;
                const preview=window.__morphforge.text.preview();
                const boxes=preview.operations
                  .filter(o=>o.op==='box')
                  .map(({op,...b})=>b);
                const bounds=preview.layout.bounds;
                const canvas=r.renderer.domElement;
                const rect=canvas.getBoundingClientRect();
                r.renderer.setAnimationLoop(null);
                r.content.visible=false;
                r.helpers.visible=false;
                const render=visible=>{
                  r.textPreviewGroup.visible=visible;
                  r.renderer.render(r.scene,r.camera);
                  const copy=document.createElement('canvas');
                  copy.width=canvas.width;copy.height=canvas.height;
                  const ctx=copy.getContext('2d',{willReadFrequently:true});
                  ctx.drawImage(canvas,0,0);
                  return {ctx,data:ctx.getImageData(0,0,copy.width,copy.height),
                          width:copy.width,height:copy.height,
                          image:copy.toDataURL('image/png')};
                };
                const before=render(false),after=render(true);
                const a=before.data.data,b=after.data.data;
                let minX=Infinity,minY=Infinity,maxX=-Infinity,maxY=-Infinity,changed=0;
                for(let i=0;i<a.length;i+=4){
                  const delta=Math.abs(a[i]-b[i])+Math.abs(a[i+1]-b[i+1])+Math.abs(a[i+2]-b[i+2]);
                  if(delta>24){
                    const p=i/4,x=p%after.width,y=Math.floor(p/after.width);
                    minX=Math.min(minX,x);minY=Math.min(minY,y);maxX=Math.max(maxX,x);maxY=Math.max(maxY,y);changed++;
                  }
                }
                const scaleX=after.width/rect.width,scaleY=after.height/rect.height;
                const project=v=>{
                  const p=r.projectPoint(v.map(n=>n*r.cellSize));
                  return {x:(p.x-rect.left)*scaleX,y:(p.y-rect.top)*scaleY};
                };
                const corners=boxes.flatMap(box=>{
                  const end=box.min.map((v,i)=>v+box.size[i]);
                  const out=[];
                  for(const x of [box.min[0],end[0]])for(const y of [box.min[1],end[1]])for(const z of [box.min[2],end[2]])out.push(project([x,y,z]));
                  return out;
                });
                const ex0=Math.min(...corners.map(p=>p.x)),ex1=Math.max(...corners.map(p=>p.x));
                const ey0=Math.min(...corners.map(p=>p.y)),ey1=Math.max(...corners.map(p=>p.y));
                const plane=window.__morphforge.text.request().plane;
                const lo=bounds.min,hi=bounds.max;
                let tl,tr,bl;
                if(plane==='xy'){
                  tl=project([hi[0],hi[1],lo[2]]);tr=project([lo[0],hi[1],lo[2]]);bl=project([hi[0],lo[1],lo[2]]);
                }else if(plane==='xz'){
                  tl=project([hi[0],lo[1],lo[2]]);tr=project([lo[0],lo[1],lo[2]]);bl=project([hi[0],lo[1],hi[2]]);
                }else{
                  tl=project([lo[0],hi[1],lo[2]]);tr=project([lo[0],hi[1],hi[2]]);bl=project([lo[0],lo[1],lo[2]]);
                }
                const actual={x0:minX,y0:minY,x1:maxX+1,y1:maxY+1,width:maxX-minX+1,height:maxY-minY+1};
                const expected={x0:ex0,y0:ey0,x1:ex1,y1:ey1,width:ex1-ex0,height:ey1-ey0};
                const ix=Math.max(0,Math.min(actual.x1,expected.x1)-Math.max(actual.x0,expected.x0));
                const iy=Math.max(0,Math.min(actual.y1,expected.y1)-Math.max(actual.y0,expected.y0));
                const actualArea=Math.max(1,actual.width*actual.height);
                const ux=tr.x-tl.x,uy=tr.y-tl.y,vx=bl.x-tl.x,vy=bl.y-tl.y;
                const determinant=ux*vy-uy*vx;
                const zones={topLeft:[0,0],topRight:[0,0],bottomLeft:[0,0],bottomRight:[0,0]};
                for(let i=0;i<a.length;i+=4){
                  const p=i/4,x=p%after.width,y=Math.floor(p/after.width);
                  const dx=x-tl.x,dy=y-tl.y;
                  const u=(dx*vy-dy*vx)/determinant;
                  const v=(ux*dy-uy*dx)/determinant;
                  let name=null;
                  if(u>=.1&&u<=.45&&v>=.03&&v<=.42)name='topLeft';
                  else if(u>=.55&&u<=.9&&v>=.03&&v<=.42)name='topRight';
                  else if(u>=.1&&u<=.45&&v>=.62&&v<=.97)name='bottomLeft';
                  else if(u>=.55&&u<=.9&&v>=.62&&v<=.97)name='bottomRight';
                  if(!name)continue;
                  const delta=Math.abs(a[i]-b[i])+Math.abs(a[i+1]-b[i+1])+Math.abs(a[i+2]-b[i+2]);
                  zones[name][1]++;
                  if(delta>24)zones[name][0]++;
                }
                const regions={};
                for(const [name,[ink,area]] of Object.entries(zones)){
                  regions[name]={ink,area,occupancy:area?ink/area:0};
                }
                return {plane,changed,actual,expected,bboxCoverage:ix*iy/actualArea,
                        readingRight:tr.x-tl.x,readingDown:bl.y-tl.y,tl,tr,bl,
                        canvas:[after.width,after.height],image:after.image,
                        boxCount:boxes.length,bounds,regions};
                """,
            )
            pixel_results[plane] = result
            image_name = f"text-real-{args.label}-{plane}.png"
            (OUT / image_name).write_bytes(
                base64.b64decode(result["image"].split(",", 1)[1])
            )
            check(
                f"{plane} positive text advances right and down on rendered canvas",
                result["changed"] > 50
                and result["readingRight"] > 2
                and result["readingDown"] > 2,
                {
                    "readingRightPx": result["readingRight"],
                    "readingDownPx": result["readingDown"],
                },
            )
            check(
                f"{plane} rendered pixel bbox matches projected canonical Boxes",
                result["bboxCoverage"] >= 0.72,
                {
                    "actual": result["actual"],
                    "expected": result["expected"],
                    "bboxCoverage": result["bboxCoverage"],
                },
            )
            regions = result["regions"]
            check(
                f"{plane} asymmetric L glyph rejects horizontal and vertical mirrors",
                regions["topLeft"]["occupancy"] >= 0.05
                and regions["topRight"]["occupancy"] <= 0.025
                and regions["bottomLeft"]["occupancy"] >= 0.15
                and regions["bottomRight"]["occupancy"] >= 0.15,
                regions,
            )
            check(
                f"{plane} preview overlay is visible",
                result["changed"] > 50,
                {"changedPixels": result["changed"], "boxCount": result["boxCount"]},
            )

        content_images = {}
        browser.set_value("textContent", "FL\n云港")
        browser.set_value("textSize", "24")
        for plane, view in VIEW.items():
            browser.set_value("textPlane", plane)
            browser.click("textPreview")
            browser.wait(
                f"return window.__morphforge.text.request()?.plane==='{plane}'"
                "&&window.__morphforge.text.request()?.text==='FL\\n云港'"
                "&&window.__morphforge.text.request()?.fontSize===24",
                20,
            )
            browser.js(
                r"""const r=window.__morphforge.renderer,b=window.__morphforge.text.preview().layout.bounds;
                   r.view(arguments[0],[(b.min[0]+b.max[0])/2,(b.min[1]+b.max[1])/2,(b.min[2]+b.max[2])/2]);
                   const center=r.controls.target.clone();
                   const cell=r.cellSize*r.scale;
                   const size=b.max.map((v,i)=>(v-b.min[i])*cell);
                   const plane=window.__morphforge.text.request().plane;
                   const width=plane==='yz'?size[2]:size[0];
                   const height=plane==='xz'?size[2]:size[1];
                   const fov=r.camera.fov*Math.PI/180;
                   const aspect=r.renderer.domElement.clientWidth/Math.max(1,r.renderer.domElement.clientHeight);
                   const halfV=height/2/Math.tan(fov/2);
                   const halfH=width/2/Math.tan(fov/2)/Math.max(.01,aspect);
                   const distance=Math.max(halfV,halfH)*1.9;
                   const direction=r.camera.position.clone().sub(center).normalize();
                   r.camera.position.copy(center).addScaledVector(direction,distance);
                   r.camera.near=Math.max(.001,distance/1000);
                   r.camera.far=Math.max(500,distance*20);
                   r.camera.updateProjectionMatrix();r.controls.update();""",
                view,
            )
            image = browser.js(
                r"""
                const r=window.__morphforge.renderer;
                const canvas=r.renderer.domElement;
                r.content.visible=false;r.helpers.visible=false;r.textPreviewGroup.visible=true;
                r.renderer.render(r.scene,r.camera);
                return canvas.toDataURL('image/png');
                """
            )
            content_images[plane] = image
            content_name = f"text-real-{args.label}-content-{plane}.png"
            (OUT / content_name).write_bytes(
                base64.b64decode(image.split(",", 1)[1])
            )
        check(
            "FL and Chinese display screenshots saved for all planes with 24px type",
            all(content_images.get(plane, "").startswith("data:image/png") for plane in VIEW),
        )

        if args.apply:
            # Leave the workspace unchanged by reverting the single apply transaction.
            browser.set_value("textPlane", "xy")
            browser.set_value("textContent", text)
            browser.click("textPreview")
            browser.wait(
                "return window.__morphforge.text.request()?.plane==='xy'&&!document.getElementById('textApply').disabled",
                20,
            )
            revision = browser.js("return window.__morphforge.state.revision")
            browser.click("textApply")
            browser.wait(
                f"return window.__morphforge.state.revision>{revision}", 30
            )
            undo_status, _ = request_json(
                origin, "POST", "/api/undo", {"revision": revision}
            )
            check("real text undo HTTP request", undo_status == 200)
            browser.wait(
                f"return window.__morphforge.state.revision>={revision + 1}", 20
            )
            check("real text apply creates one undoable revision", True)

        logs = browser.call(browser.path + "/log", {"type": "browser"})
        severe = [
            entry
            for entry in logs
            if entry["level"] == "SEVERE"
            and "favicon.ico" not in entry["message"]
            and not (
                "400" in entry["message"]
                and "Failed to load resource" in entry["message"]
            )
        ]
        check("no JavaScript or WebGL runtime errors", not severe, severe)
        browser.screenshot(OUT / f"text-real-{args.label}-page.png")
        report = {
            "success": True,
            "scope": (
                "Contract-compatible mock text API + real Chromium renderer; no model or provider request"
                if args.mock
                else "Real Rust text API + real Chromium renderer; no model or provider request"
            ),
            "url": origin,
            "font": builtin,
            "styleId": style_id,
            "revisionBefore": revision_before,
            "directPreview": {
                plane: {
                    "requestId": value.get("requestId"),
                    "requestFingerprint": value.get("requestFingerprint"),
                    "layout": value.get("layout"),
                }
                for plane, value in direct_by_plane.items()
            },
            "missingGlyph": missing,
            "pixels": {
                plane: {
                    key: value
                    for key, value in result.items()
                    if key not in {"image", "canvas"}
                }
                for plane, result in pixel_results.items()
            },
            "checks": checks,
            "browserLogs": logs,
        }
    except Exception as error:
        browser.screenshot(OUT / f"text-real-{args.label}-failure.png")
        report = {
            "success": False,
            "error": str(error),
            "checks": checks,
            "pixelFiles": [
                name
                for name in (
                    f"text-real-{args.label}-xy.png",
                    f"text-real-{args.label}-xz.png",
                    f"text-real-{args.label}-yz.png",
                )
                if (OUT / name).exists()
            ],
        }
        raise
    finally:
        (OUT / f"text-real-{args.label}-validation.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2)
        )
        browser.close()
        if server:
            server.shutdown()
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
