#!/usr/bin/env python3
"""Verify promo media, regional subtitle fonts and the reviewed timeline."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from fractions import Fraction
import json
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--render-logs', type=Path, required=True)
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
