#!/usr/bin/env python3
"""Send direct OSC Avatar parameters; standard library only, no KipSeek protocol."""

import argparse
import json
import math
from pathlib import Path
import random
import socket
import struct
import time

PREFIX = "/avatar/parameters/MFProbe_"


def osc_string(value):
    data = value.encode("utf-8") + b"\0"
    return data + b"\0" * (-len(data) % 4)


def message(name, value):
    address = osc_string(PREFIX + name)
    if isinstance(value, bool):
        return address + osc_string(",T" if value else ",F")
    value = float(value)
    if not math.isfinite(value):
        raise ValueError("OSC float must be finite")
    return address + osc_string(",f") + struct.pack(">f", value)


def bundle(values):
    packets = [message(name, value) for name, value in values]
    return b"#bundle\0" + struct.pack(">Q", 1) + b"".join(struct.pack(">i", len(p)) + p for p in packets)


def geometry(shape, seed=1):
    if shape == "tetrahedron":
        verts = [(-.65,-.5,-.5),(.65,-.5,-.5),(0,-.5,.65),(0,.7,0)]
        faces = [(0,1,2),(0,3,1),(1,3,2),(2,3,0)]
        triangles = [[verts[i] for i in face] for face in faces]
    elif shape == "roof":
        verts = [(-.75,-.3,-.6),(.75,-.3,-.6),(.75,-.3,.6),(-.75,-.3,.6),(0,.65,0)]
        faces = [(0,4,1),(1,4,2),(2,4,3),(3,4,0),(0,1,2),(0,2,3)]
        triangles = [[verts[i] for i in face] for face in faces]
    else:
        rng = random.Random(seed)
        triangles = [[tuple(rng.uniform(-.8,.8) for _ in range(3)) for _ in range(3)] for _ in range(8)]
    coordinates = [value for triangle in triangles for vertex in triangle for value in vertex]
    coordinates += [0.0] * (72-len(coordinates))
    return {"shape": shape, "seed": seed, "triangle_count": len(triangles), "coordinates": coordinates}


def validate(data):
    values = data["coordinates"]
    count = data["triangle_count"]
    if len(values) != 72 or isinstance(count, bool) or not isinstance(count, int) or not 0 <= count <= 8:
        raise ValueError("This prototype supports 72 coordinates and up to 8 triangles")
    if not all(isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v) and -1 <= v <= 1 for v in values):
        raise ValueError("Coordinates must be finite and within [-1,1]")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=9010)
    parser.add_argument("--repeat", type=int, default=3)
    sub = parser.add_subparsers(dest="command", required=True)
    signal = sub.add_parser("signal")
    signal.add_argument("--value", type=float, required=True)
    shape = sub.add_parser("geometry")
    shape.add_argument("--shape", choices=["tetrahedron", "roof", "random"], default="roof")
    shape.add_argument("--seed", type=int, default=1)
    shape.add_argument("--input", type=Path)
    shape.add_argument("--output", type=Path)
    sub.add_parser("hide")
    options = parser.parse_args()
    if options.repeat < 1:
        parser.error("--repeat must be positive")
    if options.command == "signal":
        if not 0 <= options.value <= 1:
            parser.error("Signal control must be in [0,1]")
        values = [("Control", options.value), ("Visible", True)]
        summary = {"control": options.value}
    elif options.command == "hide":
        values = [("Visible", False)]
        summary = {"visible": False}
    else:
        data = json.loads(options.input.read_text()) if options.input else geometry(options.shape, options.seed)
        validate(data)
        if options.output:
            options.output.parent.mkdir(parents=True, exist_ok=True)
            options.output.write_text(json.dumps(data, indent=2)+"\n", encoding="utf-8")
        values = [(f"V{i:02d}", value) for i, value in enumerate(data["coordinates"])]
        values += [("Count", data["triangle_count"]/8), ("Visible", True)]
        summary = {"shape": data.get("shape", "external"), "triangles": data["triangle_count"], "coordinates": 72}
    packet = bundle(values)
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sender:
        for _ in range(options.repeat):
            sender.sendto(packet, (options.host, options.port))
            time.sleep(.05)
    print(json.dumps({"sent": True, "host": options.host, "port": options.port,
                      "bytes_per_bundle": len(packet), "repeats": options.repeat, **summary}))


if __name__ == "__main__":
    main()
