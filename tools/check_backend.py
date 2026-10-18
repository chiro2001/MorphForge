#!/usr/bin/env python3
"""Read-only-safe HTTP acceptance checks for a dedicated MorphForge backend instance.

The checks only target a loopback URL supplied explicitly by the operator. They never
call the model/provider endpoint, connect/publish OSC, print response bodies, or inspect
the process environment. Run against an isolated test data directory.
"""

from __future__ import annotations

import argparse
import copy
import ipaddress
import io
import json
import re
import struct
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import zlib
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable
from xml.sax.saxutils import escape as xml_escape


ROOT = Path(__file__).resolve().parents[1]
REPORT_DIR = ROOT / "docs" / "workbench"
MAX_HTTP_BYTES = 140 * 1024 * 1024
BLOCKED_PORTS = {8766, 8767, 8768, 9012}


class CheckFailure(Exception):
    """A sanitized check failure safe to put in the report."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        return None


OPENER = urllib.request.build_opener(NoRedirect())


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise CheckFailure(reason)


def parse_target(raw: str) -> tuple[str, str, int]:
    try:
        parsed = urllib.parse.urlsplit(raw)
        host = parsed.hostname
        port = parsed.port
    except ValueError:
        raise CheckFailure("invalid_target_url") from None

    if parsed.scheme != "http" or not host or port is None:
        raise CheckFailure("target_must_be_explicit_http_loopback")
    if parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise CheckFailure("target_url_must_not_contain_credentials_or_query")
    if parsed.path not in ("", "/"):
        raise CheckFailure("target_url_must_be_an_origin_without_path")
    try:
        loopback = host.lower() == "localhost" or ipaddress.ip_address(host).is_loopback
    except ValueError:
        loopback = False
    if not loopback:
        raise CheckFailure("target_host_must_be_loopback")
    if port in BLOCKED_PORTS:
        raise CheckFailure("target_port_is_reserved")
    origin = f"http://{parsed.netloc}"
    return origin, host, port


def http_request(
    origin: str,
    method: str,
    path: str,
    *,
    json_body: Any | None = None,
    raw_body: bytes | None = None,
    headers: dict[str, str] | None = None,
    timeout: float = 25.0,
) -> tuple[int, dict[str, str], bytes]:
    request_headers = dict(headers or {})
    data = raw_body
    if json_body is not None:
        data = json.dumps(json_body, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        request_headers.setdefault("Content-Type", "application/json")
    request = urllib.request.Request(
        origin + path,
        data=data,
        headers=request_headers,
        method=method,
    )
    try:
        with OPENER.open(request, timeout=timeout) as response:
            body = response.read(MAX_HTTP_BYTES + 1)
            if len(body) > MAX_HTTP_BYTES:
                raise CheckFailure("response_exceeded_local_size_guard")
            return response.status, dict(response.headers.items()), body
    except urllib.error.HTTPError as error:
        # Read and discard the body. It may contain arbitrary backend text and is never logged.
        body = error.read(MAX_HTTP_BYTES + 1)
        if len(body) > MAX_HTTP_BYTES:
            raise CheckFailure("error_response_exceeded_local_size_guard") from None
        return error.code, dict(error.headers.items()), body
    except CheckFailure:
        raise
    except (urllib.error.URLError, TimeoutError, OSError):
        raise CheckFailure("http_transport_or_timeout") from None


def json_request(
    origin: str,
    method: str,
    path: str,
    *,
    body: Any | None = None,
    timeout: float = 25.0,
) -> tuple[int, Any | None]:
    status, _, raw = http_request(origin, method, path, json_body=body, timeout=timeout)
    if not raw:
        return status, None
    try:
        return status, json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        if status < 400:
            raise CheckFailure("expected_json_response") from None
        return status, None


def snapshot(origin: str) -> tuple[dict[str, Any], dict[str, Any]]:
    status, state = json_request(origin, "GET", "/api/state")
    require(status == 200 and isinstance(state, dict), "state_endpoint_unavailable")
    status, scene = json_request(origin, "GET", "/api/scene")
    require(status == 200 and isinstance(scene, dict), "scene_endpoint_unavailable")
    require(isinstance(scene.get("scene"), dict), "scene_payload_missing_scene")
    require(isinstance(scene.get("materials"), list), "scene_payload_missing_materials")
    return state, scene


def check_malformed_scene_import_atomicity(origin: str) -> dict[str, Any]:
    cases: list[tuple[str, Callable[[int], dict[str, Any]]]] = [
        ("empty_scene", lambda revision: {"scene": {}, "baseRevision": revision}),
        (
            "arbitrary_json_scene",
            lambda revision: {
                "scene": {"arbitrary": "json"},
                "baseRevision": revision,
            },
        ),
        (
            "unknown_scene_format",
            lambda revision: {
                "scene": {"format": "Unknown/9", "boxes": []},
                "baseRevision": revision,
            },
        ),
        (
            "empty_nested_project_scene",
            lambda revision: {
                "project": {"scene": {}},
                "baseRevision": revision,
            },
        ),
        (
            "zero_size_scene",
            lambda revision: {
                "scene": {
                    "format": "MorphForgeScene/1",
                    "cellSize": 0.25,
                    "size": 0,
                    "boxes": [],
                },
                "baseRevision": revision,
            },
        ),
    ]
    results: list[dict[str, Any]] = []
    for name, make_body in cases:
        before_state, before_scene = snapshot(origin)
        status, _ = json_request(
            origin,
            "POST",
            "/api/import",
            body=make_body(before_state["revision"]),
        )
        after_state, after_scene = snapshot(origin)
        revision_unchanged = before_state["revision"] == after_state["revision"]
        scene_unchanged = before_scene == after_scene
        results.append(
            {
                "case": name,
                "httpStatus": status,
                "revisionUnchanged": revision_unchanged,
                "sceneAndMaterialsUnchanged": scene_unchanged,
                "passed": status == 400 and revision_unchanged and scene_unchanged,
            }
        )
    passed = all(result["passed"] for result in results)
    return {
        "_passed": passed,
        "allMalformedScenesRejectedAtomically": passed,
        "cases": results,
    }


def check_record(report: dict[str, Any], name: str, run: Callable[[], dict[str, Any]]) -> None:
    started = time.monotonic()
    try:
        details = run()
        passed = bool(details.pop("_passed", True))
        result = {
            "name": name,
            "status": "pass" if passed else "fail",
            "elapsedMs": round((time.monotonic() - started) * 1000),
            "details": details,
        }
    except CheckFailure as error:
        result = {
            "name": name,
            "status": "fail",
            "elapsedMs": round((time.monotonic() - started) * 1000),
            "reason": str(error),
        }
    except Exception as error:
        # Do not serialize arbitrary exceptions or payloads into the report.
        result = {
            "name": name,
            "status": "fail",
            "elapsedMs": round((time.monotonic() - started) * 1000),
            "reason": f"internal_{type(error).__name__}",
        }
    report["checks"].append(result)


def check_bad_material_import(origin: str) -> dict[str, Any]:
    before_state, before_scene = snapshot(origin)
    materials = before_scene["materials"]
    require(bool(materials), "no_material_available_for_duplicate_id_probe")
    duplicate = copy.deepcopy(materials[0])
    status, _ = json_request(
        origin,
        "POST",
        "/api/import",
        body={
            "scene": before_scene["scene"],
            "materials": [copy.deepcopy(duplicate), duplicate],
            "baseRevision": before_state["revision"],
        },
    )
    after_root_state, after_root_scene = snapshot(origin)
    root_revision_unchanged = before_state["revision"] == after_root_state["revision"]
    root_scene_and_materials_unchanged = before_scene == after_root_scene

    nested_before_state, nested_before_scene = after_root_state, after_root_scene
    nested_duplicate = copy.deepcopy(nested_before_scene["materials"][0])
    nested_status, _ = json_request(
        origin,
        "POST",
        "/api/import",
        body={
            "project": {
                "scene": nested_before_scene["scene"],
                "materials": [copy.deepcopy(nested_duplicate), nested_duplicate],
            },
            "baseRevision": nested_before_state["revision"],
        },
    )
    after_nested_state, after_nested_scene = snapshot(origin)
    nested_revision_unchanged = (
        nested_before_state["revision"] == after_nested_state["revision"]
    )
    nested_scene_and_materials_unchanged = nested_before_scene == after_nested_scene
    passed = (
        status == 400
        and root_revision_unchanged
        and root_scene_and_materials_unchanged
        and nested_status == 400
        and nested_revision_unchanged
        and nested_scene_and_materials_unchanged
    )
    return {
        "_passed": passed,
        "rootMaterialsHttpStatus": status,
        "rootRevisionUnchanged": root_revision_unchanged,
        "rootSceneAndMaterialsUnchanged": root_scene_and_materials_unchanged,
        "nestedProjectMaterialsHttpStatus": nested_status,
        "nestedProjectRevisionUnchanged": nested_revision_unchanged,
        "nestedProjectSceneAndMaterialsUnchanged": nested_scene_and_materials_unchanged,
    }


def check_project_atomicity(origin: str) -> dict[str, Any]:
    before_state, before_scene = snapshot(origin)
    status, _ = json_request(
        origin,
        "POST",
        "/api/project",
        body={
            "name": "BACKEND_VALIDATION_INVALID_PROJECT_SENTINEL",
            "size": 0,
            "baseRevision": before_state["revision"],
        },
    )
    after_state, after_scene = snapshot(origin)
    name_unchanged = (
        before_state.get("project", {}).get("name")
        == after_state.get("project", {}).get("name")
    )
    revision_unchanged = before_state["revision"] == after_state["revision"]
    scene_unchanged = before_scene == after_scene
    passed = status == 400 and name_unchanged and revision_unchanged and scene_unchanged
    return {
        "_passed": passed,
        "httpStatus": status,
        "nameUnchanged": name_unchanged,
        "revisionUnchanged": revision_unchanged,
        "sceneUnchanged": scene_unchanged,
    }


def cell_is_free(scene: dict[str, Any], cell: list[int]) -> bool:
    for box in scene.get("boxes", []):
        minimum = box["min"]
        size = box["size"]
        if all(minimum[i] <= cell[i] < minimum[i] + size[i] for i in range(3)):
            return False
    return True


def choose_free_cell(scene: dict[str, Any]) -> list[int]:
    # Deterministic and inside the normal avatar coordinate envelope.
    candidates = (
        [x, y, z]
        for y in range(256)
        for x in range(-128, 128)
        for z in range(-128, 128)
    )
    for cell in candidates:
        if cell_is_free(scene, cell):
            return cell
    # Dedicated test projects should be sparse; this fallback remains inside the core limit.
    for i in range(10000):
        cell = [500000 + i, 500000, 500000]
        if cell_is_free(scene, cell):
            return cell
    raise CheckFailure("no_free_cell_for_request_id_probe")


def check_recipe_host_capabilities(origin: str) -> dict[str, Any]:
    before_state, before_scene = snapshot(origin)
    require(bool(before_scene["materials"]), "no_material_available_for_recipe_probe")
    style_id = int(before_scene["materials"][0]["id"])
    cell = choose_free_cell(before_scene["scene"])
    names = [
        "process",
        "require",
        "fetch",
        "XMLHttpRequest",
        "WebSocket",
        "Deno",
        "Bun",
        "window",
        "document",
        "localStorage",
        "fs",
        "env",
    ]
    code = (
        "const names="
        + json.dumps(names)
        + ";"
        + "const exposed=names.filter(n=>typeof globalThis[n]!==\"undefined\");"
        + "if(exposed.length)throw new Error(\"host capability exposed\");"
        + f"mf.box({json.dumps(cell)},[1,1,1],{{style:{style_id}}});"
    )
    status, result = json_request(
        origin,
        "POST",
        "/api/harness/tool",
        body={
            "name": "run_recipe",
            "arguments": {
                "code": code,
                "mode": "apply",
                "baseRevision": before_state["revision"],
                "seed": 17,
                "maxOps": 4,
                "dryRun": True,
            },
        },
        timeout=25.0,
    )
    after_state, after_scene = snapshot(origin)
    dry_run_ok = status == 200 and isinstance(result, dict) and result.get("dryRun") is True
    unchanged = before_state["revision"] == after_state["revision"] and before_scene == after_scene
    passed = dry_run_ok and unchanged
    return {
        "_passed": passed,
        "httpStatus": status,
        "hostFilesystemNetworkAndEnvironmentGlobalsAbsent": dry_run_ok,
        "dryRunDidNotChangeGeometry": unchanged,
    }


def run_malicious_recipe(origin: str, code: str) -> dict[str, Any]:
    before_state, before_scene = snapshot(origin)
    require(bool(before_scene["materials"]), "no_material_available_for_malicious_recipe")
    style_id = int(before_scene["materials"][0]["id"])
    cell = choose_free_cell(before_scene["scene"])
    staged_code = f"mf.box({json.dumps(cell)},[1,1,1],{{style:{style_id}}});\n{code}"
    status, _ = json_request(
        origin,
        "POST",
        "/api/harness/tool",
        body={
            "name": "run_recipe",
            "arguments": {
                "code": staged_code,
                "mode": "apply",
                "baseRevision": before_state["revision"],
                "seed": 23,
                "maxOps": 4,
                "dryRun": False,
            },
        },
        timeout=25.0,
    )
    after_state, after_scene = snapshot(origin)
    geometry_unchanged = (
        before_state["revision"] == after_state["revision"]
        and before_scene == after_scene
    )
    failed_cleanly = status == 400 and geometry_unchanged
    return {
        "_passed": failed_cleanly,
        "httpStatus": status,
        "failedWithoutApplyingStagedGeometry": geometry_unchanged,
        "responseBodyLogged": False,
    }


def check_request_id_idempotency_and_revision_conflict(origin: str) -> dict[str, Any]:
    before_state, before_scene = snapshot(origin)
    require(bool(before_scene["materials"]), "no_material_available_for_edit_probe")
    style_id = int(before_scene["materials"][0]["id"])
    cell = choose_free_cell(before_scene["scene"])
    request_id = "backend-check-" + uuid.uuid4().hex
    body = {
        "op": "box",
        "min": cell,
        "size": [1, 1, 1],
        "styleId": style_id,
        "baseRevision": before_state["revision"],
        "requestId": request_id,
    }
    first_status, first = json_request(origin, "POST", "/api/edit", body=body)
    require(first_status == 200 and isinstance(first, dict), "first_idempotent_edit_failed")
    after_first, scene_after_first = snapshot(origin)
    duplicate_status, duplicate = json_request(origin, "POST", "/api/edit", body=body)
    after_duplicate, scene_after_duplicate = snapshot(origin)
    duplicate_replayed = (
        duplicate_status == 200
        and duplicate == first
        and after_duplicate["revision"] == after_first["revision"]
        and scene_after_duplicate == scene_after_first
    )

    conflict_body = dict(body)
    conflict_body.update(
        {
            "min": choose_free_cell(scene_after_duplicate["scene"]),
            "baseRevision": after_duplicate["revision"] + 1,
            "requestId": request_id + "-stale",
        }
    )
    conflict_status, _ = json_request(
        origin,
        "POST",
        "/api/edit",
        body=conflict_body,
    )
    after_conflict, scene_after_conflict = snapshot(origin)
    conflict_preserved = (
        conflict_status == 409
        and after_conflict["revision"] == after_duplicate["revision"]
        and scene_after_conflict == scene_after_duplicate
    )
    return {
        "_passed": duplicate_replayed and conflict_preserved,
        "firstEditHttpStatus": first_status,
        "duplicateHttpStatus": duplicate_status,
        "duplicateRequestIdReplayedWithoutNewRevision": duplicate_replayed,
        "staleRevisionHttpStatus": conflict_status,
        "revisionConflictPreservedScene": conflict_preserved,
    }


def png_rgba_1x1(color: tuple[int, int, int, int]) -> bytes:
    def chunk(kind: bytes, data: bytes) -> bytes:
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    header = struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0)
    pixel = b"\x00" + bytes(color)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(pixel))
        + chunk(b"IEND", b"")
    )


def java_test_pack() -> bytes:
    files = {
        "pack.mcmeta": json.dumps(
            {"pack": {"pack_format": 46, "description": "MorphForge backend validation"}}
        ),
        "assets/backendcheck/textures/block/checker.png": png_rgba_1x1(
            (71, 139, 203, 255)
        ),
        "assets/backendcheck/models/block/checker.json": json.dumps(
            {
                "parent": "minecraft:block/cube_all",
                "textures": {"all": "block/checker"},
            }
        ),
    }
    output = io.BytesIO()
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for path, contents in files.items():
            archive.writestr(path, contents)
    return output.getvalue()


def multipart_file(field: str, filename: str, contents: bytes) -> tuple[bytes, str]:
    boundary = "----MorphForgeBackendCheck" + uuid.uuid4().hex
    body = (
        f"--{boundary}\r\n"
        f'Content-Disposition: form-data; name="{field}"; filename="{filename}"\r\n'
        "Content-Type: application/zip\r\n\r\n"
    ).encode("ascii") + contents + f"\r\n--{boundary}--\r\n".encode("ascii")
    return body, f"multipart/form-data; boundary={boundary}"


def check_catalog_texture_references(origin: str) -> dict[str, Any]:
    data = java_test_pack()
    body, content_type = multipart_file("file", "backend-check.zip", data)
    status, _, raw = http_request(
        origin,
        "POST",
        "/api/packs/import",
        raw_body=body,
        headers={"Content-Type": content_type},
        timeout=25.0,
    )
    require(status == 200, "resource_pack_import_failed")
    try:
        imported = json.loads(raw.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        raise CheckFailure("resource_pack_import_response_invalid") from None
    pack_id = imported.get("packId")
    require(isinstance(pack_id, str) and len(pack_id) == 64, "resource_pack_id_missing")

    catalog_status, _, catalog_bytes = http_request(
        origin,
        "GET",
        "/api/vrc/catalog",
        timeout=25.0,
    )
    require(catalog_status == 200, "catalog_export_failed")
    try:
        with zipfile.ZipFile(io.BytesIO(catalog_bytes)) as archive:
            names = set(archive.namelist())
            require("catalog.json" in names, "catalog_manifest_missing")
            catalog = json.loads(archive.read("catalog.json").decode("utf-8"))
            materials = catalog.get("materials", [])
            pack_materials = [m for m in materials if m.get("pack") == pack_id]
            refs = []
            invalid_refs = 0
            for material in materials:
                for texture in material.get("textures", []):
                    if not isinstance(texture, str) or not texture.startswith("/artifacts/"):
                        invalid_refs += 1
                    else:
                        refs.append("artifacts/" + texture.removeprefix("/artifacts/"))
            missing = [ref for ref in refs if ref not in names]
    except (zipfile.BadZipFile, KeyError, UnicodeDecodeError, json.JSONDecodeError):
        raise CheckFailure("catalog_archive_invalid") from None

    has_pack_texture = bool(pack_materials) and any(
        material.get("textures") for material in pack_materials
    )
    all_references_present = invalid_refs == 0 and not missing and bool(refs)
    passed = has_pack_texture and all_references_present
    return {
        "_passed": passed,
        "packMaterialCount": len(pack_materials),
        "textureReferenceCount": len(refs),
        "allReferencesResolveInsideCatalogZip": all_references_present,
        "invalidReferenceCount": invalid_refs,
        "missingReferenceCount": len(missing),
    }


def diagnostics_secret_pattern_found(data: bytes) -> bool:
    patterns = (
        re.compile(
            rb"(?im)^\s*(?:export\s+)?(?:DEEPSEEK_API_KEY|OPENAI_API_KEY)\s*[:=]\s*\S+"
        ),
        re.compile(
            rb"(?i)[\"']?(?:DEEPSEEK_API_KEY|OPENAI_API_KEY|api[_-]?key)[\"']?\s*[:=]\s*[\"']?[A-Za-z0-9._~+/=-]{12,}"
        ),
        re.compile(rb"(?i)\bsk-[A-Za-z0-9_-]{20,}\b"),
        re.compile(rb"(?i)authorization\s*[:=]\s*[\"']?bearer\s+[A-Za-z0-9._~+/=-]{16,}"),
    )
    return any(pattern.search(data) for pattern in patterns)


def check_diagnostics_redaction(origin: str) -> dict[str, Any]:
    status, _, data = http_request(origin, "GET", "/api/diagnostics", timeout=30.0)
    require(status == 200, "diagnostics_export_failed")
    try:
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            names = archive.namelist()
            lower_names = [name.lower() for name in names]
            forbidden_names = (
                ".env",
                "provider.env",
                "provider-files.json",
                "capture-browser",
                "browser-profile",
                "chrome-profile",
                "edge-profile",
            )
            secret_files_absent = not any(
                any(token in name for token in forbidden_names) for name in lower_names
            )
            text_members = [
                name
                for name in names
                if name != "manifest.json"
                and name.lower().endswith((".json", ".jsonl", ".js", ".txt"))
            ]
            credential_pattern_absent = True
            for name in text_members:
                if archive.getinfo(name).file_size > 32 * 1024 * 1024:
                    credential_pattern_absent = False
                    break
                if diagnostics_secret_pattern_found(archive.read(name)):
                    credential_pattern_absent = False
                    break
            manifest = (
                json.loads(archive.read("manifest.json").decode("utf-8"))
                if "manifest.json" in names
                else {}
            )
            excluded = {str(value).lower() for value in manifest.get("excluded", [])}
            manifest_excludes_sensitive_data = (
                any("credential" in value for value in excluded)
                and any("browser" in value for value in excluded)
                and any("provider-files" in value for value in excluded)
            )
    except (zipfile.BadZipFile, KeyError, UnicodeDecodeError, json.JSONDecodeError):
        raise CheckFailure("diagnostics_archive_invalid") from None

    passed = secret_files_absent and credential_pattern_absent and manifest_excludes_sensitive_data
    return {
        "_passed": passed,
        "memberCount": len(names),
        "credentialAndEnvironmentFilesAbsent": secret_files_absent,
        "credentialLikeContentAbsent": credential_pattern_absent,
        "browserProfileFilesAbsent": secret_files_absent,
        "manifestDeclaresSensitiveExclusions": manifest_excludes_sensitive_data,
    }


def build_report_path(label: str) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return REPORT_DIR / f"backend-validation-{label}-{stamp}.json"


def write_report(path: Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", required=True, help="Dedicated loopback backend URL, e.g. http://127.0.0.1:8769")
    parser.add_argument("--label", required=True, help="Report label such as linux or windows")
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,31}", args.label):
        parser.error("--label must contain only letters, digits, _ or -")

    report_path = build_report_path(args.label)
    report: dict[str, Any] = {
        "format": "MorphForgeBackendValidation/1",
        "label": args.label,
        "startedAt": datetime.now(timezone.utc).isoformat(),
        "target": {},
        "safety": {
            "providerEndpointCalled": False,
            "oscEndpointCalled": False,
            "diagnosticPayloadsOrSecretsPrinted": False,
        },
        "checks": [],
    }

    try:
        origin, host, port = parse_target(args.url)
        report["target"] = {"host": host, "port": port, "scheme": "http"}
        status, state = json_request(origin, "GET", "/api/state")
        if status != 200 or not isinstance(state, dict):
            raise CheckFailure("dedicated_test_backend_not_ready")
        if not isinstance(state.get("revision"), int):
            raise CheckFailure("state_revision_missing")
    except CheckFailure as error:
        report["checks"].append(
            {"name": "target_health", "status": "fail", "reason": str(error)}
        )
        report["overall"] = "fail"
        report["finishedAt"] = datetime.now(timezone.utc).isoformat()
        write_report(report_path, report)
        print(
            json.dumps(
                {
                    "overall": report["overall"],
                    "report": str(report_path.relative_to(ROOT)),
                    "checks": [{"name": c["name"], "status": c["status"]} for c in report["checks"]],
                },
                ensure_ascii=False,
            )
        )
        return 1

    report["checks"].append(
        {
            "name": "target_health",
            "status": "pass",
            "details": {"httpStatus": status, "revisionPresent": True},
        }
    )
    checks: list[tuple[str, Callable[[], dict[str, Any]]]] = [
        ("malformed_scene_import_atomicity", lambda: check_malformed_scene_import_atomicity(origin)),
        ("bad_material_import_atomicity", lambda: check_bad_material_import(origin)),
        ("invalid_project_settings_atomicity", lambda: check_project_atomicity(origin)),
        ("recipe_host_capability_isolation", lambda: check_recipe_host_capabilities(origin)),
        (
            "infinite_recipe_failure_rolls_back_geometry",
            lambda: run_malicious_recipe(
                origin,
                "while(true){}",
            ),
        ),
        (
            "memory_limit_failure_rolls_back_geometry",
            lambda: run_malicious_recipe(
                origin,
                "const blocks=[];for(let i=0;i<100000;i++){blocks.push('x'.repeat(1048576));}",
            ),
        ),
        (
            "duplicate_request_id_and_revision_conflict",
            lambda: check_request_id_idempotency_and_revision_conflict(origin),
        ),
        ("catalog_zip_texture_references", lambda: check_catalog_texture_references(origin)),
        ("diagnostics_redaction_and_exclusions", lambda: check_diagnostics_redaction(origin)),
    ]
    for name, check in checks:
        check_record(report, name, check)

    report["finishedAt"] = datetime.now(timezone.utc).isoformat()
    report["overall"] = (
        "pass" if all(check["status"] == "pass" for check in report["checks"]) else "fail"
    )
    write_report(report_path, report)
    print(
        json.dumps(
            {
                "overall": report["overall"],
                "report": str(report_path.relative_to(ROOT)),
                "checks": [
                    {"name": check["name"], "status": check["status"]}
                    for check in report["checks"]
                ],
            },
            ensure_ascii=False,
        )
    )
    return 0 if report["overall"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())

