#!/usr/bin/env python3
"""Verify promo media, regional subtitle fonts and the reviewed timeline."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from fractions import Fraction
import json
import hashlib
from pathlib import Path
import re
import subprocess


def probe(path):
    return json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_format', '-show_streams', '-of', 'json', str(path)]))


def seconds(value):
    h, m, s = value.split(':')
    return int(h) * 3600 + int(m) * 60 + float(s.replace(',', '.'))


def cues(path):
    value = re.sub(r'<[^>]+>', '', path.read_text(encoding='utf-8-sig')).strip()
    result = []
    for block in re.split(r'\n\s*\n', value):
        lines = block.splitlines()
        start, end = (seconds(t) for t in lines[1].split(' --> '))
        result.append((int(lines[0]), start, end, '\n'.join(lines[2:])))
    return result


def verify_localized(plan, args, check):
    duration, frames = plan["duration"], round(plan["duration"] * 60)
    check("Two independent Chinese and Japanese picture editions", plan["editions"] == ["zh-CN", "ja"] and len(list(args.out.glob("MorphForge-Promo-*-1080p60.mp4"))) == 2)
    check("Timeline frame counts equal declared duration", sum(round(s["duration"] * 60) for s in plan["shots"]) == frames)
    mlp_seconds = sum(s["duration"] for s in plan["shots"] if s.get("file") == "neural-network.mp4")
    galleries = [s for s in plan["shots"] if s["kind"] == "gallery"]
    check("MLP reduced to one 3.2-second example", abs(mlp_seconds - 3.2) < .001)
    check("Six saved artworks in three image pairs", len(galleries) == 3 and len({p for s in galleries for p in s["images"]}) == 6)
    check("Low-poly tree uses original camera motion without stabilization", any(s.get("file") == "stabilize.mp4" and not s.get("stabilize") and s["duration"] == 6.4 for s in plan["shots"]))
    typography = json.loads((args.out / "typography.json").read_text())
    localization = json.loads((args.out / "localization.json").read_text())
    paths = {c: args.out / f"MorphForge-Promo-{c}-1080p60.mp4" for c in plan["editions"]}
    for language, path in paths.items():
        data = probe(path)
        v = next(s for s in data["streams"] if s["codec_type"] == "video")
        a = next(s for s in data["streams"] if s["codec_type"] == "audio")
        check(f"{language}: 1080p60 H.264 and exact frame count", v["codec_name"] == "h264" and (v["width"], v["height"]) == (1920, 1080) and Fraction(v["avg_frame_rate"]) == 60 and int(v["nb_frames"]) == frames)
        check(f"{language}: exact picture and container duration", abs(float(data["format"]["duration"]) - duration) < .03 and abs(float(v["duration"]) - duration) < .02)
        check(f"{language}: picture and 48 kHz stereo AAC without subtitle tracks", len(data["streams"]) == 2 and a["codec_name"] == "aac" and a["sample_rate"] == "48000" and a["channels"] == 2)
        check(f"{language}: video language metadata", v["tags"]["language"] == ("jpn" if language == "ja" else "zho"))
        family = "Noto Sans CJK JP" if language == "ja" else "Noto Sans CJK SC"
        check(f"{language}: correct regular and bold regional font faces", all(typography[language][w]["family"] == family for w in ["regular", "bold"]) and localization[language]["fontFamily"] == family)
        actual = json.loads((args.out / f"render-plan.{language}.json").read_text())["shots"]
        valid = len(actual) == len(plan["shots"])
        for original, rendered in zip(plan["shots"], actual):
            expected_title = original.get("titleJa", "") if language == "ja" else original["title"]
            expected_source = original.get("sourceLabelJa", "") if language == "ja" else original.get("sourceLabel", "")
            valid = valid and rendered["id"] == original["id"] and rendered["language"] == language and rendered["title"] == expected_title and rendered.get("sourceLabel", "") == expected_source
        check(f"{language}: all graphic titles and source labels localized", valid)
        check(f"{language}: localized cover and storyboard exist", (args.out / f"Cover-{language}.jpg").stat().st_size > 10000 and (args.out / f"Storyboard-{language}.jpg").stat().st_size > 10000)
    different = []
    for shot in plan["shots"]:
        suffix = ".png" if shot["kind"] == "end" else "-heading.png" if shot["title"] else "-necessary.png" if shot.get("sourceLabel") else None
        if suffix:
            images = [args.render_logs / code / "graphics" / (shot["id"] + suffix) for code in plan["editions"]]
            different.append(images[0].read_bytes() != images[1].read_bytes())
    check("Localized title, gallery-label and end-card graphics differ between editions", len(different) == 16 and all(different), {"localizedGraphics": len(different)})
    check("Chinese and Japanese covers are separate artwork", (args.out / "Cover-zh-CN.jpg").read_bytes() != (args.out / "Cover-ja.jpg").read_bytes())
    records = json.loads((args.out / "artwork-sources.json").read_text())["artworks"]
    check("Six bundled artwork images retain recorded SHA256", len(records) == 6 and all(hashlib.sha256((args.out / "assets" / r["file"]).read_bytes()).hexdigest() == r["sha256"] for r in records))
    for lang in plan["subtitleLanguages"]:
        actual, expected, position = cues(args.out / f"MorphForge-Promo.{lang}.srt"), [], 0
        for shot in plan["shots"]:
            message = shot.get("captionZh", shot.get("title", "")) if lang == "zh-CN" else shot[lang]
            if message and shot["kind"] != "brand":
                expected.append((len(expected) + 1, position + .15, position + min(shot["duration"] - .15, .15 + shot.get("captionSeconds", 2.4)), message))
            position += shot["duration"]
        check(f"{lang}: optional sidecar text and timing match timeline", len(actual) == len(expected) and all(a[0] == e[0] and abs(a[1]-e[1]) <= .001 and abs(a[2]-e[2]) <= .001 and a[3] == e[3] for a,e in zip(actual,expected)), {"cues": len(actual)})
    def decode(code):
        r = subprocess.run(["ffmpeg", "-v", "error", "-xerror", "-threads", "4", "-i", str(paths[code]), "-map", "0:v:0", "-map", "0:a:0", "-sn", "-f", "null", "-"], capture_output=True, text=True)
        (args.work / f"decode-{code}.log").write_text(r.stderr)
        return code, r
    with ThreadPoolExecutor(max_workers=2) as pool:
        for code, result in pool.map(decode, plan["editions"]):
            check(f"{code}: complete picture and sound decode", result.returncode == 0, result.stderr.strip() or None)
    if args.sources:
        tree = next(s for s in plan["shots"] if s.get("file") == "stabilize.mp4")
        source = args.sources / tree["file"]
        if not source.exists():
            source = args.sources / plan["sources"][tree["file"]]
        tree_position = sum(s["duration"] for s in plan["shots"][:plan["shots"].index(tree)])
        expected_frames = round(tree["duration"] * 60)
        for code, path in paths.items():
            stats_file = args.work / f"tree-ssim-{code}.txt"
            safe_path = stats_file.resolve().as_posix().replace(":", "\\:").replace("'", "\\'")
            graph = f"[0:v]setpts=PTS-STARTPTS,fps=60,crop=1920:900:0:180[original];[1:v]setpts=PTS-STARTPTS,crop=1920:900:0:180[edited];[original][edited]ssim=stats_file='{safe_path}'"
            result = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-filter_complex_threads", "2", "-threads", "4", "-ss", str(tree["start"]), "-t", str(tree["duration"]), "-i", str(source), "-ss", str(tree_position), "-t", str(tree["duration"]), "-i", str(path), "-filter_complex", graph, "-an", "-frames:v", str(expected_frames), "-f", "null", "-"], capture_output=True, text=True, check=True)
            (args.work / f"tree-ssim-{code}.log").write_text(result.stderr)
            values = [float(x) for x in re.findall(r"All:([0-9.]+)", stats_file.read_text())]
            mean = sum(values) / len(values)
            check(f"{code}: entire tree sequence matches original geometry", len(values) == expected_frames and mean >= .98 and min(values) >= .95, {"comparedFrames": len(values), "averageSsim": mean, "minimumSsim": min(values), "region": "1920x900 below title area", "method": "Every delivered tree frame compared to the corresponding source frame; no spatial transformation or interpolation expected."})
    for code,path in paths.items():
        result = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", str(path), "-map", "0:a:0", "-vn", "-af", "loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json", "-f", "null", "-"], capture_output=True, text=True, check=True)
        (args.work / f"loudness-{code}.log").write_text(result.stderr)
        stats = json.loads(result.stderr[result.stderr.rfind("{"):result.stderr.rfind("}") + 1])
        check(f"{code}: music loudness and true peak within range", -19 <= float(stats["input_i"]) <= -14 and float(stats["input_tp"]) < -.2, stats)
    return dict(durationSeconds=duration, frames=frames, shotCount=len(plan["shots"]), bytes={code:p.stat().st_size for code,p in paths.items()}, mlpSeconds=mlp_seconds, gallerySeconds=sum(s["duration"] for s in galleries), sourceAudio="muted", note="Independent localized graphic layers; source GUI/world lettering remains authentic. Automated full decode and sampled visual review do not imply a human viewing/listening to the entire film.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--render-logs', type=Path, required=True)
    parser.add_argument('--sources', type=Path, help='Compare the entire tree sequence to original footage')
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=True)
    plan = json.loads((args.out / 'timeline.json').read_text())
    duration = plan['duration']
    frames = round(duration * plan['fps'])
    checks = []

    def check(name, passed, detail=None):
        checks.append(dict(name=name, passed=bool(passed), detail=detail))
        print(name, bool(passed), flush=True)
        if not passed:
            raise AssertionError(name)

    if plan.get("editions"):
        details = verify_localized(plan, args, check)
        report = dict(status="passed", checks=checks, **details)
        (args.out / "verification.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
        print(f"{len(checks)} checks passed", flush=True)
        return
    paths = {code: args.out / f'MorphForge-Promo-{code}-1080p60.mp4' for code in ['zh-CN', 'ja', 'Multisub']}
    probes = {code: probe(path) for code, path in paths.items()}
    check('Timeline frame counts equal declared duration', sum(round(s['duration'] * 60) for s in plan['shots']) == frames)
    for code, data in probes.items():
        v = next(s for s in data['streams'] if s['codec_type'] == 'video')
        a = next(s for s in data['streams'] if s['codec_type'] == 'audio')
        check(f'{code}: 1080p60 H.264, exact frame count', v['codec_name'] == 'h264' and (v['width'], v['height']) == (1920, 1080) and Fraction(v['avg_frame_rate']) == 60 and int(v['nb_frames']) == frames)
        check(f'{code}: exact timeline duration', abs(float(data['format']['duration']) - duration) < .03 and abs(float(v['duration']) - duration) < .02)
        check(f'{code}: 48 kHz stereo AAC', a['codec_name'] == 'aac' and a['sample_rate'] == '48000' and a['channels'] == 2)
    check('Chinese and Japanese masters contain only picture and sound', all(len(probes[c]['streams']) == 2 for c in ['zh-CN', 'ja']))
    subs = [s for s in probes['Multisub']['streams'] if s['codec_type'] == 'subtitle']
    check('Companion contains Japanese and English mov_text tracks', len(subs) == 2 and [s['tags']['language'] for s in subs] == ['jpn', 'eng'] and all(s['codec_name'] == 'mov_text' for s in subs))
    for lang in plan['subtitleLanguages']:
        expected = []
        position = 0
        for shot in plan['shots']:
            message = shot.get('captionZh', shot.get('title', '')) if lang == 'zh-CN' else shot[lang]
            if message and shot['kind'] != 'brand':
                expected.append((len(expected) + 1, position + .15, position + min(shot['duration'] - .15, .15 + shot.get('captionSeconds', 2.4)), message))
            position += shot['duration']
        actual = cues(args.out / f'MorphForge-Promo.{lang}.srt')
        check(f'{lang}: subtitle text and times match timeline', len(actual) == len(expected) and all(a[0] == e[0] and abs(a[1] - e[1]) <= .001 and abs(a[2] - e[2]) <= .001 and a[3] == e[3] for a, e in zip(actual, expected)), {'cues': len(actual)})
        check(f'{lang}: captions are ordered and within picture', all(0 <= a[1] < a[2] <= duration and (i == 0 or a[1] >= actual[i-1][2]) for i, a in enumerate(actual)))
    for index, lang in enumerate(['ja', 'en']):
        target = args.work / f'extracted-{lang}.srt'
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(paths['Multisub']), '-map', f'0:s:{index}', '-c:s', 'srt', str(target)], check=True)
        actual, expected = cues(target), cues(args.out / f'MorphForge-Promo.{lang}.srt')
        check(f'{lang}: embedded subtitles preserve text and millisecond timing', len(actual) == len(expected) and all(a[3] == e[3] and abs(a[1]-e[1]) <= .002 and abs(a[2]-e[2]) <= .002 for a, e in zip(actual, expected)))
    typography = json.loads((args.out / 'typography.json').read_text())
    for code, family in [('zh-CN', 'Noto Sans CJK SC'), ('ja', 'Noto Sans CJK JP')]:
        check(f'{code}: explicit regional family in regular/bold fonts and ASS', all(typography[code][w]['family'] == family for w in ['regular', 'bold']) and f'Style: Default,{family},' in (args.out / f'MorphForge-Promo.{code}.ass').read_text())
    burn_log = (args.render_logs / 'japanese-font-burn.log').read_text(errors='replace')
    check('Japanese burned subtitles resolve to JP glyphs without missing-glyph warnings', 'fontselect: (Noto Sans CJK JP, 400, 0) -> NotoSansCJKjp-Regular' in burn_log and 'Glyph ' not in burn_log)
    payload = paths['Multisub'].read_bytes()
    check('Selectable subtitles retain SC/JP font family names', b'Noto Sans CJK JP' in payload and b'Noto Sans CJK SC' in payload)

    def decode(code):
        r = subprocess.run(['ffmpeg', '-v', 'error', '-xerror', '-threads', '4', '-i', str(paths[code]), '-map', '0:v:0', '-map', '0:a:0', '-sn', '-f', 'null', '-'], capture_output=True, text=True)
        (args.work / f'decode-{code}.log').write_text(r.stderr)
        return code, r

    with ThreadPoolExecutor(max_workers=2) as pool:
        for code, result in pool.map(decode, ['zh-CN', 'ja']):
            check(f'{code}: full picture and sound decode', result.returncode == 0, result.stderr.strip() or None)
    loud = subprocess.run(['ffmpeg', '-hide_banner', '-nostats', '-i', str(paths['zh-CN']), '-map', '0:a:0', '-vn', '-af', 'loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json', '-f', 'null', '-'], capture_output=True, text=True, check=True)
    (args.work / 'loudness.log').write_text(loud.stderr)
    stats = json.loads(loud.stderr[loud.stderr.rfind('{'):loud.stderr.rfind('}') + 1])
    check('Music integrated loudness and true peak within range', -19 <= float(stats['input_i']) <= -14 and float(stats['input_tp']) < -.2, stats)
    report = dict(status='passed', checks=checks, durationSeconds=duration, frames=frames, shotCount=len(plan['shots']), bytes={code: p.stat().st_size for code,p in paths.items()}, sourceAudio='muted', note='Full automated decode and sampled visual review; no claim of a human watching/listening to the whole film.')
    (args.out / 'verification.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(f'{len(checks)} checks passed', flush=True)


if __name__ == '__main__':
    main()
