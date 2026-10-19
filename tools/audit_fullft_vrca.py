#!/usr/bin/env python3
"""Audit the actual FullFT VRCA bundle rather than the SDK build report.

Requires UnityPy. The script intentionally reads only bundle contents and never
downloads files, launches VRChat, or sends OSC.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any


DEFAULT_PARAMETERS = [
    "MF3_Ctl",
    "MF3_D00",
    "MF3_D01",
    "MF3_D02",
    "MF3_D03",
    "MF3_D04",
    "MF3_D05",
    "MF_World",
    "MF_Handle",
    "MF_Scale",
    "MF_Reset",
    "MF_GrabL",
    "MF_GrabR",
    "MF_Level",
    "FT/AutoToggle",
    "FT/v2/EyeLeftX",
    "FT/v2/EyeRightX",
    "FT/v2/JawOpen",
    "FT/v2/MouthClosed",
    "EyeTrackingActive",
    "LipTrackingActive",
]


def read_tree(obj: Any) -> dict[str, Any] | None:
    try:
        value = obj.read_typetree()
    except Exception:
        return None
    return value if isinstance(value, dict) else None


def object_name(value: dict[str, Any] | None) -> str | None:
    if not value:
        return None
    name = value.get("m_Name")
    return str(name) if name is not None else None


def path_id(reference: Any) -> int | None:
    if not isinstance(reference, dict):
        return None
    value = reference.get("m_PathID")
    return int(value) if value is not None else None


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def add_check(
    checks: list[dict[str, Any]],
    name: str,
    passed: bool,
    actual: Any,
    expected: Any,
) -> None:
    checks.append(
        {
            "name": name,
            "pass": bool(passed),
            "actual": actual,
            "expected": expected,
        }
    )


def texture_record(
    env: Any,
    objects: dict[int, Any],
    reference: Any,
) -> dict[str, Any] | None:
    texture_id = path_id(reference)
    if texture_id is None or texture_id not in objects:
        return None
    obj = objects[texture_id]
    tree = read_tree(obj)
    if not tree:
        return None
    return {
        "pathId": texture_id,
        "name": tree.get("m_Name"),
        "width": tree.get("m_Width"),
        "height": tree.get("m_Height"),
        "format": tree.get("m_TextureFormat"),
        "mipCount": tree.get("m_MipCount"),
        "colorSpace": tree.get("m_ColorSpace"),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bundle", type=Path)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--expected-capacity", type=int, default=16384)
    parser.add_argument(
        "--expected-shader",
        default="MorphForge/ParticleBoxCatalog",
    )
    parser.add_argument(
        "--expected-metadata",
        default="256x64",
        help="WIDTHxHEIGHT for the material's _CatalogMeta texture",
    )
    parser.add_argument(
        "--expected-atlas",
        default="384x96",
        help="WIDTHxHEIGHT for the material's _CatalogAtlas texture",
    )
    parser.add_argument(
        "--forbidden-collision-mask",
        type=lambda value: int(value, 0),
        default=(1 << 13) | (1 << 14),
        help=(
            "VRChat LayerHelper particle-collision forbidden mask; "
            "default is Pickup(13) | PickupNoEnvironment(14)"
        ),
    )
    parser.add_argument(
        "--parameter",
        action="append",
        dest="parameters",
        help="Required serialized parameter name; repeatable",
    )
    parser.add_argument(
        "--skip-parameter-strings",
        action="store_true",
        help="For diagnostic bundles that predate the FullFT manifest",
    )
    parser.add_argument(
        "--budget-json",
        type=Path,
        help="SDK final-budget JSON used as the exact 112-item manifest",
    )
    return parser.parse_args()


def parse_size(text: str) -> tuple[int, int]:
    try:
        width, height = text.lower().split("x", 1)
        return int(width), int(height)
    except Exception as exc:
        raise argparse.ArgumentTypeError("Expected WIDTHxHEIGHT") from exc


def main() -> int:
    args = parse_args()
    if not args.bundle.is_file():
        raise SystemExit(f"Bundle not found: {args.bundle}")
    budget = (
        json.loads(args.budget_json.read_text(encoding="utf-8"))
        if args.budget_json
        else None
    )
    try:
        import UnityPy  # type: ignore
    except ImportError as exc:
        raise SystemExit(
            "UnityPy is required: python -m pip install UnityPy"
        ) from exc

    expected_metadata = parse_size(args.expected_metadata)
    expected_atlas = parse_size(args.expected_atlas)
    env = UnityPy.load(str(args.bundle))
    objects = {int(obj.path_id): obj for obj in env.objects}
    trees: dict[int, dict[str, Any] | None] = {
        pid: read_tree(obj) for pid, obj in objects.items()
    }

    game_objects: dict[int, str] = {}
    for pid, obj in objects.items():
        if obj.type.name == "GameObject":
            name = object_name(trees.get(pid))
            if name is not None:
                game_objects[pid] = name

    systems: list[dict[str, Any]] = []
    collision_force_systems: list[dict[str, Any]] = []
    forbidden_collision_force_systems: list[dict[str, Any]] = []
    renderers: dict[int, dict[str, Any]] = {}
    for pid, obj in objects.items():
        tree = trees.get(pid)
        if not tree:
            continue
        game_object = path_id(tree.get("m_GameObject"))
        if obj.type.name == "ParticleSystem":
            initial = tree.get("InitialModule") or {}
            lifetime = (initial.get("startLifetime") or {}).get("scalar")
            lifetime_text = (
                "Infinity"
                if isinstance(lifetime, (int, float))
                and math.isinf(float(lifetime))
                and float(lifetime) > 0
                else "-Infinity"
                if isinstance(lifetime, (int, float))
                and math.isinf(float(lifetime))
                else lifetime
            )
            systems.append(
                {
                    "pathId": pid,
                    "gameObjectId": game_object,
                    "name": game_objects.get(game_object),
                    "capacity": initial.get("maxNumParticles"),
                    "startLifetime": lifetime_text,
                    "infinityLifetime": (
                        isinstance(lifetime, (int, float))
                        and math.isinf(float(lifetime))
                        and float(lifetime) > 0
                    ),
                    "size3D": initial.get("size3D"),
                    "burstCount": (tree.get("EmissionModule") or {}).get(
                        "m_BurstCount"
                    ),
                }
            )
            collision = tree.get("CollisionModule") or {}
            collider_force = collision.get("colliderForce", 0)
            if (
                isinstance(collider_force, (int, float))
                and collider_force != 0
            ):
                collides_with = (
                    collision.get("collidesWith") or {}
                ).get("m_Bits", 0)
                record = {
                    "pathId": pid,
                    "gameObjectId": game_object,
                    "name": game_objects.get(game_object),
                    "collisionEnabled": collision.get("enabled"),
                    "collisionType": collision.get("type"),
                    "colliderForce": collider_force,
                    "collidesWith": collides_with,
                    "collisionMessages": collision.get(
                        "collisionMessages"
                    ),
                }
                collision_force_systems.append(record)
                if (
                    collision.get("type") == 1
                    and (
                        int(collides_with or 0)
                        & args.forbidden_collision_mask
                    )
                    != 0
                ):
                    forbidden_collision_force_systems.append(record)
        elif obj.type.name == "ParticleSystemRenderer":
            renderer = {
                "pathId": pid,
                "gameObjectId": game_object,
                "name": game_objects.get(game_object),
                "materialIds": [
                    path_id(item)
                    for item in tree.get("m_Materials", [])
                    if path_id(item) is not None
                ],
            }
            if game_object is not None:
                renderers[game_object] = renderer

    stores = [item for item in systems if item["name"] == "Store"]
    pumps = [item for item in systems if item["name"] == "Pump"]
    checks: list[dict[str, Any]] = []
    add_check(checks, "exactly one Store", len(stores) == 1, len(stores), 1)
    store = stores[0] if len(stores) == 1 else None
    if store:
        add_check(
            checks,
            "Store capacity",
            store["capacity"] == args.expected_capacity,
            store["capacity"],
            args.expected_capacity,
        )
        add_check(
            checks,
            "Store lifetime is +Infinity",
            store["infinityLifetime"],
            store["startLifetime"],
            "Infinity",
        )
        add_check(
            checks,
            "Store uses 3D particle size",
            store["size3D"] is True,
            store["size3D"],
            True,
        )
        add_check(
            checks,
            "Store has one burst",
            store["burstCount"] == 1,
            store["burstCount"],
            1,
        )

    material_record: dict[str, Any] | None = None
    metadata: dict[str, Any] | None = None
    atlas: dict[str, Any] | None = None
    renderer = (
        renderers.get(store["gameObjectId"]) if store else None
    )
    add_check(
        checks,
        "Store renderer exists",
        renderer is not None,
        renderer is not None,
        True,
    )
    if renderer and renderer["materialIds"]:
        material_id = renderer["materialIds"][0]
        material = trees.get(material_id)
        if material:
            shader_id = path_id(material.get("m_Shader"))
            shader_tree = trees.get(shader_id) if shader_id else None
            shader_name = None
            if shader_tree:
                shader_name = (
                    (shader_tree.get("m_ParsedForm") or {}).get("m_Name")
                    or shader_tree.get("m_Name")
                )
            saved = material.get("m_SavedProperties") or {}
            tex_envs: dict[str, Any] = {}
            for pair in saved.get("m_TexEnvs", []):
                if isinstance(pair, (list, tuple)) and len(pair) == 2:
                    tex_envs[str(pair[0])] = pair[1].get("m_Texture")
            floats = {
                str(pair[0]): pair[1]
                for pair in saved.get("m_Floats", [])
                if isinstance(pair, (list, tuple)) and len(pair) == 2
            }
            metadata = texture_record(
                env, objects, tex_envs.get("_CatalogMeta")
            )
            atlas = texture_record(
                env, objects, tex_envs.get("_CatalogAtlas")
            )
            material_record = {
                "pathId": material_id,
                "name": material.get("m_Name"),
                "shaderPathId": shader_id,
                "shader": shader_name,
                "metaWidth": floats.get("_MetaWidth"),
                "metaHeight": floats.get("_MetaHeight"),
                "textures": sorted(tex_envs),
            }
            add_check(
                checks,
                "Store shader",
                shader_name == args.expected_shader,
                shader_name,
                args.expected_shader,
            )
            add_check(
                checks,
                "Material _CatalogMeta texture",
                metadata is not None,
                metadata is not None,
                True,
            )
            add_check(
                checks,
                "Material _CatalogAtlas texture",
                atlas is not None,
                atlas is not None,
                True,
            )
            add_check(
                checks,
                "_MetaWidth property",
                floats.get("_MetaWidth") == expected_metadata[0],
                floats.get("_MetaWidth"),
                expected_metadata[0],
            )
            add_check(
                checks,
                "_MetaHeight property",
                floats.get("_MetaHeight") == expected_metadata[1],
                floats.get("_MetaHeight"),
                expected_metadata[1],
            )

    if metadata:
        add_check(
            checks,
            "metadata dimensions",
            (metadata["width"], metadata["height"]) == expected_metadata,
            [metadata["width"], metadata["height"]],
            list(expected_metadata),
        )
        add_check(
            checks,
            "metadata RGBAFloat format 20",
            metadata["format"] == 20,
            metadata["format"],
            20,
        )
    if atlas:
        add_check(
            checks,
            "atlas dimensions",
            (atlas["width"], atlas["height"]) == expected_atlas,
            [atlas["width"], atlas["height"]],
            list(expected_atlas),
        )

    texture_records = []
    for pid, obj in objects.items():
        if obj.type.name != "Texture2D":
            continue
        tree = trees.get(pid)
        if tree:
            texture_records.append(
                {
                    "pathId": pid,
                    "name": tree.get("m_Name"),
                    "width": tree.get("m_Width"),
                    "height": tree.get("m_Height"),
                    "format": tree.get("m_TextureFormat"),
                }
            )
    original_metadata_paths = [
        item
        for item in texture_records
        if item["width"] == 16384 and item["height"] == 1
    ]
    add_check(
        checks,
        "no 16384x1 metadata texture",
        not original_metadata_paths,
        original_metadata_paths,
        [],
    )

    mono_scripts = sorted(
        {
            name
            for pid, obj in objects.items()
            if obj.type.name == "MonoScript"
            and (name := object_name(trees.get(pid)))
        }
    )
    morphforge_runtime_scripts = [
        name for name in mono_scripts if "MorphForge" in name
    ]
    add_check(
        checks,
        "no MorphForge runtime MonoScript",
        not morphforge_runtime_scripts,
        morphforge_runtime_scripts,
        [],
    )

    expression_parameters: list[dict[str, Any]] = []
    expression_parameter_asset: dict[str, Any] | None = None
    parameter_script_ids = {
        pid
        for pid, obj in objects.items()
        if obj.type.name == "MonoScript"
        and (
            object_name(trees.get(pid)) == "VRCExpressionParameters"
            or (trees.get(pid) or {}).get("m_ClassName")
            == "VRCExpressionParameters"
        )
    }
    for pid, obj in objects.items():
        if obj.type.name != "MonoBehaviour":
            continue
        tree = trees.get(pid)
        if not tree or path_id(tree.get("m_Script")) not in parameter_script_ids:
            continue
        raw_parameters = tree.get("parameters")
        if not isinstance(raw_parameters, list):
            continue
        expression_parameter_asset = {
            "pathId": pid,
            "name": tree.get("m_Name"),
            "isEmpty": tree.get("isEmpty"),
        }
        for parameter in raw_parameters:
            if not isinstance(parameter, dict):
                continue
            value_type = int(parameter.get("valueType", -1))
            expression_parameters.append(
                {
                    "name": parameter.get("name"),
                    "valueType": value_type,
                    "type": {0: "Int", 1: "Float", 2: "Bool"}.get(
                        value_type, "Unknown"
                    ),
                    "bits": 1 if value_type == 2 else 8,
                    "saved": bool(parameter.get("saved")),
                    "defaultValue": parameter.get("defaultValue"),
                    "networkSynced": bool(parameter.get("networkSynced")),
                }
            )
        break

    parameter_by_name = {
        item["name"]: item
        for item in expression_parameters
        if item["name"]
    }
    synced_parameters = [
        item for item in expression_parameters if item["networkSynced"]
    ]
    synced_bits = sum(int(item["bits"]) for item in synced_parameters)
    add_check(
        checks,
        "exactly one VRCExpressionParameters asset",
        expression_parameter_asset is not None,
        expression_parameter_asset,
        "one",
    )

    group_summaries: dict[str, Any] = {}
    if budget is not None:
        groups = {
            "ft": (
                budget.get("ft", {}).get("entries", []),
                161,
                "FT/",
            ),
            "controls": (
                budget.get("controls", {}).get("entries", []),
                8,
                "",
            ),
            "geometry": (
                budget.get("morphForge", {})
                .get("geometry", {})
                .get("entries", []),
                56,
                "MF3_",
            ),
            "placement": (
                budget.get("morphForge", {})
                .get("placement", {})
                .get("entries", []),
                14,
                "MF_",
            ),
        }
        for group_name, (entries, expected_bits, prefix) in groups.items():
            actual_entries: list[dict[str, Any]] = []
            failures: list[str] = []
            for expected in entries:
                name = expected.get("name")
                expected_type = expected.get("expectedType")
                expected_count = int(expected.get("expectedBits", 0))
                actual = parameter_by_name.get(name)
                if actual is None:
                    failures.append(f"missing {name}")
                    continue
                actual_entries.append(
                    {
                        "name": name,
                        "type": actual["type"],
                        "bits": actual["bits"],
                        "networkSynced": actual["networkSynced"],
                        "expectedType": expected_type,
                        "expectedBits": expected_count,
                    }
                )
                if actual["type"] != expected_type:
                    failures.append(
                        f"{name} type {actual['type']} != {expected_type}"
                    )
                if actual["bits"] != expected_count:
                    failures.append(
                        f"{name} bits {actual['bits']} != {expected_count}"
                    )
                if not actual["networkSynced"]:
                    failures.append(f"{name} is not networkSynced")
            group_bits = sum(
                int(item["bits"]) for item in actual_entries
            )
            if len(actual_entries) != len(entries):
                failures.append(
                    f"count {len(actual_entries)} != {len(entries)}"
                )
            if group_bits != expected_bits:
                failures.append(
                    f"bits {group_bits} != {expected_bits}"
                )
            if prefix:
                for item in actual_entries:
                    if not item["name"].startswith(prefix):
                        failures.append(
                            f"{item['name']} lacks prefix {prefix}"
                        )
            add_check(
                checks,
                f"{group_name} exact manifest, types, sync and bits",
                not failures,
                {
                    "count": len(actual_entries),
                    "bits": group_bits,
                    "failures": failures,
                },
                {
                    "count": len(entries),
                    "bits": expected_bits,
                    "failures": [],
                },
            )
            group_summaries[group_name] = {
                "count": len(actual_entries),
                "bits": group_bits,
                "failures": failures,
                "entries": actual_entries,
            }
        add_check(
            checks,
            "actual expression parameter total is 256 bits",
            synced_bits == 256,
            synced_bits,
            256,
        )
        add_check(
            checks,
            "SDK final budget reports 256 bits",
            budget.get("finalBits") == 256
            and budget.get("maxBits") == 256
            and budget.get("remainingBits") == 0,
            {
                "finalBits": budget.get("finalBits"),
                "maxBits": budget.get("maxBits"),
                "remainingBits": budget.get("remainingBits"),
            },
            {"finalBits": 256, "maxBits": 256, "remainingBits": 0},
        )

    serialized_terms: set[str] = set()
    for obj in objects.values():
        try:
            raw = obj.get_raw_data()
        except Exception:
            continue
        for term in (args.parameters or DEFAULT_PARAMETERS):
            if term.encode("utf-8") in raw:
                serialized_terms.add(term)
    if not args.skip_parameter_strings:
        required = args.parameters or DEFAULT_PARAMETERS
        missing = sorted(set(required) - serialized_terms)
        add_check(
            checks,
            "required parameter names serialized",
            not missing,
            {"present": sorted(set(required) & serialized_terms)},
            {"missing": []},
        )
    else:
        missing = []

    pump = pumps[0] if len(pumps) == 1 else None
    add_check(
        checks,
        "exactly one Pump",
        len(pumps) == 1,
        len(pumps),
        1,
    )
    other_systems = [
        item for item in systems
        if item["name"] not in {"Store", "Pump"}
    ]
    if pump:
        add_check(
            checks,
            "Pump capacity is 1",
            pump["capacity"] == 1,
            pump["capacity"],
            1,
        )
    add_check(
        checks,
        "no World particle collision force on forbidden Pickup/Item layers",
        not forbidden_collision_force_systems,
        forbidden_collision_force_systems,
        [],
    )

    report = {
        "schemaVersion": "1.0",
        "scope": "Actual VRCA bundle audit; no download, VRChat launch, or OSC",
        "success": all(item["pass"] for item in checks),
        "bundle": {
            "path": str(args.bundle),
            "bytes": args.bundle.stat().st_size,
            "sha256": sha256_file(args.bundle),
        },
        "expectations": {
            "capacity": args.expected_capacity,
            "shader": args.expected_shader,
            "metadata": list(expected_metadata),
            "atlas": list(expected_atlas),
            "parameters": (
                [] if args.skip_parameter_strings
                else sorted(set(args.parameters or DEFAULT_PARAMETERS))
            ),
        },
        "particleSystems": systems,
        "particleCollisionForceSystems": collision_force_systems,
        "forbiddenCollisionForceSystems": forbidden_collision_force_systems,
        "forbiddenCollisionMask": args.forbidden_collision_mask,
        "otherCharacterEffects": {
            "count": len(other_systems),
            "totalCapacity": sum(
                int(item["capacity"] or 0) for item in other_systems
            ),
            "systems": other_systems,
            "separateFromStoreLimit": True,
        },
        "storeRenderer": renderer,
        "storeMaterial": material_record,
        "metadata": metadata,
        "atlas": atlas,
        "textures": sorted(texture_records, key=lambda item: item["pathId"]),
        "monoScripts": mono_scripts,
        "morphForgeRuntimeScripts": morphforge_runtime_scripts,
        "expressionParameters": {
            "asset": expression_parameter_asset,
            "totalParameterCount": len(expression_parameters),
            "syncedParameterCount": len(synced_parameters),
            "syncedBits": synced_bits,
            "groups": group_summaries,
            "parameters": expression_parameters,
        },
        "serializedParameterNames": sorted(serialized_terms),
        "missingParameterNames": missing,
        "checks": checks,
        "failedChecks": [
            item["name"] for item in checks if not item["pass"]
        ],
    }
    rendered = json.dumps(
        report,
        indent=2,
        ensure_ascii=False,
        allow_nan=False,
    ) + "\n"
    if args.json_out:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(rendered, encoding="utf-8")
    print(rendered, end="")
    return 0 if report["success"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
