#!/usr/bin/env python3
"""Build the reviewed MorphForge promo from user footage and real product captures.

Requires ffmpeg/ffprobe with libx264, libass and vid.stab, and Pillow.
No application API, OSC, paid model or external publishing operation is used.
"""
import argparse
import hashlib
import json
import shutil
import subprocess
from functools import lru_cache
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont, ImageOps

ROOT = Path(__file__).resolve().parents[1]
if (Path(__file__).resolve().parent / "assets").is_dir():
    ROOT = Path(__file__).resolve().parent
W, H = 1920, 1080
BG = (7, 13, 21)
WHITE = (239, 244, 250)
MUTED = (163, 181, 197)
GOLD = (235, 197, 111)
MINT = (109, 222, 190)


def asset(relative):
    for path in [ROOT / relative, ROOT / Path(relative).name, ROOT / "assets" / Path(relative).name]:
        if path.exists():
            return path
    raise FileNotFoundError(relative)


def filter_path(path):
    return path.resolve().as_posix().replace(":", "\\:").replace("'", "\\'")


def run(command, log):
    with log.open("wb") as stream:
        result = subprocess.run(command, stdout=stream, stderr=subprocess.STDOUT)
    if result.returncode:
        raise RuntimeError(f"Command failed ({result.returncode}); see {log}:\n" + log.read_text(errors="replace")[-4500:])


def probe(path):
    return json.loads(subprocess.check_output(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(path)]))


@lru_cache(maxsize=8)
def font_face(language="zh-CN", bold=False):
    # A TTC contains several regional faces. Index 0 in Noto CJK is Japanese;
    # selecting the file alone silently gives Chinese text Japanese glyphs.
    if language not in {"zh-CN", "ja", "en"}:
        raise ValueError(f"Unsupported typography language: {language}")
    expected = "Noto Sans CJK JP" if language == "ja" else "Noto Sans CJK SC"
    weight = "Bold" if bold else "Regular"
    filename = f"NotoSansCJK-{weight}.ttc"
    for path in [ROOT / "assets" / filename, Path("/usr/share/fonts/noto-cjk") / filename]:
        if not path.exists():
            continue
        for index in range(10):
            try:
                face = ImageFont.truetype(str(path), 24, index=index)
            except OSError:
                break
            if face.getname() == (expected, weight):
                return str(path), index, expected, weight
    raise RuntimeError(f"Required regional font face missing: {expected} {weight}")


@lru_cache(maxsize=128)
def font(size, bold=False, language="zh-CN"):
    path, index, _, _ = font_face(language, bold)
    return ImageFont.truetype(path, size, index=index)


def text(draw, xy, content, size=38, fill=WHITE, bold=False, max_width=None, language="zh-CN"):
    runs = [(language, content)]
    # This explicit Japanese language name also occurs within Chinese copy.
    if language == "zh-CN" and "日本語" in content:
        pieces = content.split("日本語")
        runs = []
        for index, piece in enumerate(pieces):
            if index:
                runs.append(("ja", "日本語"))
            runs.append(("zh-CN", piece))
    width = sum(draw.textlength(value, font=font(size, bold, code)) for code, value in runs)
    if max_width and width > max_width:
        raise ValueError(f"Text exceeds its safe area: {content}")
    x, y = xy
    for code, value in runs:
        face = font(size, bold, code)
        draw.text((x, y), value, font=face, fill=fill, stroke_width=0)
        x += draw.textlength(value, font=face)


def canvas():
    image = Image.new("RGB", (W, H), BG)
    draw = ImageDraw.Draw(image)
    for x in range(-H, W, 100):
        draw.line((x, H, x + H, 0), fill=(10, 22, 31), width=1)
    draw.rounded_rectangle((1390, -180, 2160, 920), radius=270, fill=(10, 24, 32))
    return image


def header(image, shot):
    draw = ImageDraw.Draw(image)
    if shot.get("title"):
        width = int(draw.textlength(shot["title"], font=font(42, True))) + 48
        draw.rounded_rectangle((62, 48, 62 + width, 127), radius=12, fill=(*BG, 205))
        text(draw, (86, 61), shot["title"], 42, WHITE, True, max_width=1600)
    return draw


def card(image, path, box, crop=None, label=None, cover=False, label_language="zh-CN"):
    draw = ImageDraw.Draw(image)
    x, y, width, height = box
    source = Image.open(path).convert("RGB")
    if crop:
        source = source.crop(crop)
    source = ImageOps.fit(source, (width, height), method=Image.Resampling.LANCZOS) if cover else ImageOps.contain(source, (width, height), method=Image.Resampling.LANCZOS)
    draw.rounded_rectangle((x - 2, y - 2, x + width + 2, y + height + 2), radius=13, fill=(43, 63, 77))
    draw.rectangle((x, y, x + width, y + height), fill=(5, 10, 17))
    image.paste(source, (x + (width - source.width) // 2, y + (height - source.height) // 2))
    if label:
        text(draw, (x + 13, y - 44), label, 26, MINT, language=label_language)


def video_overlay(shot, index, total):
    image = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    header(image, shot)
    return image


def necessary_overlay(shot):
    image = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    label = shot.get("speedLabel") or shot.get("sourceLabel")
    if label:
        draw = ImageDraw.Draw(image)
        width = int(draw.textlength(label, font=font(27))) + 34
        draw.rounded_rectangle((W - width - 62, 59, W - 62, 112), radius=8, fill=(*BG, 205))
        text(draw, (W - width - 45, 66), label, 27, WHITE)
    return image


def still_card(shot):
    image = canvas()
    draw = ImageDraw.Draw(image)
    media = "docs/releases/0.27.0/media/"
    if shot["kind"] == "workbench":
        card(image, asset(media + "leviathan-lantern-editor.png"), (170, 130, 1580, 850), crop=(430, 220, 1200, 1090))
    elif shot["kind"] == "drawing":
        thin = asset("docs/optimization-20261023/drawing-thin.png")
        thick = asset("docs/optimization-20261023/drawing-thick.png")
        card(image, thin, (170, 250, 650, 650))
        card(image, thick, (1100, 250, 650, 650))
        draw.line((900, 575, 1018, 575), fill=MINT, width=8)
        draw.polygon([(1018, 575), (991, 556), (991, 594)], fill=MINT)
    elif shot["kind"] == "languages":
        for i, (code, label) in enumerate([("zh-CN", "中文"), ("ja", "日本語"), ("en", "English")]):
            x = 70 + i * 620
            draw.rounded_rectangle((x, 340, x + 540, 710), radius=24, fill=(13, 25, 35), outline=(41, 62, 74), width=2)
            width = draw.textlength(label, font=font(68, True, code))
            text(draw, (x + (540 - width) / 2, 465), label, 68, WHITE, True, language=code)
    elif shot["kind"] == "publish":
        source = asset("docs/default-motion/native/main-publish-with-motion.png")
        card(image, source, (1370, 370, 480, 100), crop=(1290, 500, 1771, 576))
        card(image, source, (1420, 570, 380, 85), crop=(1290, 584, 1526, 627))
    elif shot["kind"] == "end":
        text(draw, (115, 180), "MorphForge", 125, WHITE, True)
        draw.rectangle((122, 374, 280, 380), fill=GOLD)
        text(draw, (123, 430), shot["title"], 60, WHITE, True, 1720)
        text(draw, (123, 850), "Music: Cipher — Kevin MacLeod (incompetech.com) · CC BY 4.0", 25, MUTED, max_width=1680)
        text(draw, (123, 903), "creativecommons.org/licenses/by/4.0/ · edited excerpt", 24, MUTED)
    else:
        raise ValueError(shot["kind"])
    return image


def timestamp(seconds):
    ms = round(seconds * 1000)
    return f"{ms//3600000:02}:{ms//60000%60:02}:{ms//1000%60:02},{ms%1000:03}"


def write_subtitles(plan, out):
    for language in plan["subtitleLanguages"]:
        blocks = []
        position = 0.0
        for shot in plan["shots"]:
            start = position + .15
            end = position + min(shot["duration"] - .15, .15 + shot.get("captionSeconds", 2.4))
            position += shot["duration"]
            message = shot.get("captionZh", shot.get("title", "")) if language == "zh-CN" else shot[language]
            if not message or shot["kind"] == "brand":
                continue
            blocks.append(f"{len(blocks)+1}\n{timestamp(start)} --> {timestamp(end)}\n{message}\n")
        (out / f"MorphForge-Promo.{language}.srt").write_text("\n".join(blocks), encoding="utf-8")
        # SRT has no font information. ASS carries an explicit regional family;
        # the same style is passed into the MP4 subtitle encoder and libass.
        family = font_face(language)[2]
        header = f"""[Script Info]
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,{family},42,&H00FFFFFF,&H000000FF,&H0010151D,&H80000000,0,0,0,0,100,100,0,0,1,2.4,0.5,2,70,70,30,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"""
        events = []
        for block in blocks:
            lines = block.splitlines()
            start, end = lines[1].split(" --> ")
            def ass_time(value):
                return value[1:].replace(",", ".")[:-1]
            message = r"\N".join(lines[2:]).replace("{", r"\{").replace("}", r"\}")
            events.append(f"Dialogue: 0,{ass_time(start)},{ass_time(end)},Default,,0,0,0,,{message}\n")
        (out / f"MorphForge-Promo.{language}.ass").write_text(header + "".join(events), encoding="utf-8")


def encode_args():
    return ["-an", "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-pix_fmt", "yuv420p", "-r", "60", "-g", "120", "-threads", "4", "-color_primaries", "bt709", "-color_trc", "bt709", "-colorspace", "bt709", "-movflags", "+faststart"]


def render_shot(shot, index, total, args, work):
    destination = work / "segments" / (shot["id"] + ".mp4")
    signature = hashlib.sha256(json.dumps(shot, ensure_ascii=False, sort_keys=True).encode() + Path(__file__).read_bytes()).hexdigest()
    stamp = destination.with_suffix(".signature")
    if destination.exists() and stamp.exists() and stamp.read_text() == signature:
        print("Reuse", shot["id"], flush=True)
        return destination
    duration = shot["duration"]
    command = ["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-filter_complex_threads", "2", "-filter_threads", "2", "-threads", "4"]
    filters = []
    cursor = 1
    if shot["kind"] in {"workbench", "drawing", "languages", "end"}:
        image = work / "graphics" / (shot["id"] + ".png")
        still_card(shot).save(image)
        frames = round(duration * 60)
        command += ["-i", str(image)]
        effect = f"zoompan=z='1+0.012*on/{frames}':x='iw/2-iw/zoom/2':y='ih/2-ih/zoom/2':d={frames}:s=1920x1080:fps=60,setsar=1"
        if shot["kind"] == "end":
            effect += f",fade=t=out:st={duration-.4}:d=0.4"
        filters.append(f"[0:v]{effect}[base]")
    else:
        source = args.sources / shot["file"]
        if not source.exists():
            source = args.sources / args.plan_data["sources"][shot["file"]]
        speed = shot.get("speed", 1)
        effect = ""
        if shot.get("stabilize"):
            transform = work / "tree.trf"
            run(["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-threads", "4", "-ss", str(shot["start"]), "-t", str(duration * speed), "-i", str(source), "-vf", f"vidstabdetect=shakiness=6:accuracy=12:stepsize=8:fileformat=ascii:result='{filter_path(transform)}'", "-an", "-f", "null", "-"], work / "logs/stabilize-detect.log")
            effect = f"vidstabtransform=input='{filter_path(transform)}':smoothing=24:optzoom=1:zoom=2:interpol=bicubic,"
        effect += f"setpts=(PTS-STARTPTS)/{speed},fps=60,"
        if shot.get("crop"):
            x, y, width, height = shot["crop"]
            effect += f"crop={width}:{height}:{x}:{y},"
        # Preserve the source's geometry when focusing on part of a real GUI.
        effect += "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:color=0x070d15,setsar=1"
        command += ["-ss", str(shot["start"]), "-t", str(duration * speed), "-i", str(source)]
        if shot["kind"] == "publish":
            image = work / "graphics" / (shot["id"] + "-panel.png")
            still_card(shot).save(image)
            command += ["-loop", "1", "-framerate", "60", "-i", str(image)]
            cursor += 1
            filters.append(f"[0:v]{effect},scale=1260:708[scene]")
            filters.append("[1:v][scene]overlay=55:195:shortest=1[base]")
        else:
            filters.append(f"[0:v]{effect}[base]")
    current = "base"
    if shot["kind"] == "brand" or (shot.get("title") and shot["kind"] != "end"):
        overlay = video_overlay(shot, index, total)
        if shot["kind"] == "brand":
            overlay = Image.new("RGBA", (W, H), (0, 0, 0, 0))
            draw = ImageDraw.Draw(overlay)
            draw.rounded_rectangle((65, 755, 1005, 970), radius=20, fill=(*BG, 210))
            text(draw, (105, 775), "MorphForge", 120, WHITE, True)
        image = work / "graphics" / (shot["id"] + "-heading.png")
        overlay.save(image)
        command += ["-loop", "1", "-framerate", "60", "-i", str(image)]
        fade = "fade=t=in:st=0:d=0.12:alpha=1"
        if shot["kind"] != "brand":
            fade += f",fade=t=out:st={min(shot.get('headingSeconds',2.2),duration)-.25}:d=0.25:alpha=1"
        filters.append(f"[{cursor}:v]{fade}[heading]")
        filters.append(f"[{current}][heading]overlay=0:0:shortest=1[titled]")
        current = "titled"
        cursor += 1
    if shot.get("speedLabel") or shot.get("sourceLabel"):
        image = work / "graphics" / (shot["id"] + "-necessary.png")
        necessary_overlay(shot).save(image)
        command += ["-loop", "1", "-framerate", "60", "-i", str(image)]
        filters.append(f"[{current}][{cursor}:v]overlay=0:0:shortest=1[qualified]")
        current = "qualified"
    command += ["-filter_complex", ";".join(filters), "-map", f"[{current}]", "-t", str(duration), *encode_args(), str(destination)]
    print("Render", shot["id"], flush=True)
    run(command, work / "logs" / (shot["id"] + ".log"))
    stamp.write_text(signature)
    return destination


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, default=asset("docs/promo-video/20261025/timeline.json"))
    parser.add_argument("--sources", type=Path, required=True)
    parser.add_argument("--music", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--out", type=Path, default=ROOT / "Releases/MorphForge-Promo-20261025")
    parser.add_argument("--only", help="Render only this shot for a visual check")
    args = parser.parse_args()
    plan = json.loads(args.plan.read_text())
    args.plan_data = plan
    assert sum(round(s["duration"] * 60) for s in plan["shots"]) == round(plan["duration"] * 60)
    assert (plan["width"], plan["height"], plan["fps"]) == (W, H, 60)
    for folder in [args.out, args.work / "segments", args.work / "graphics", args.work / "logs"]:
        folder.mkdir(parents=True, exist_ok=True)
    for shot in plan["shots"]:
        if "file" not in shot:
            continue
        source = args.sources / shot["file"]
        if not source.exists():
            source = args.sources / plan["sources"][shot["file"]]
        info = probe(source)
        assert shot["start"] >= 0 and shot["start"] + shot["duration"] * shot.get("speed", 1) <= float(info["format"]["duration"]), shot["id"]
        if shot.get("crop"):
            stream = next(s for s in info["streams"] if s["codec_type"] == "video")
            x, y, width, height = shot["crop"]
            assert min(x, y) >= 0 and min(width, height) > 0 and x + width <= stream["width"] and y + height <= stream["height"], shot["id"]
    segments = []
    for index, shot in enumerate(plan["shots"]):
        if args.only and args.only != shot["id"]:
            continue
        segments.append(render_shot(shot, index, len(plan["shots"]), args, args.work))
    if args.only:
        return
    write_subtitles(plan, args.out)
    concat = args.work / "concat.txt"
    concat.write_text("".join("file '" + p.resolve().as_posix().replace("'", "'\\''") + "'\n" for p in segments))
    picture = args.work / "picture.mp4"
    run(["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-f", "concat", "-safe", "0", "-i", str(concat), "-an", "-c:v", "copy", str(picture)], args.work / "logs/concat.log")
    measure = args.work / "logs/loudness-measure.log"
    run(["ffmpeg", "-hide_banner", "-y", "-ss", str(plan["music"]["start"]), "-t", str(plan["duration"]), "-i", str(args.music), "-af", "loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json", "-f", "null", "-"], measure)
    log = measure.read_text(errors="replace")
    loudness = json.loads(log[log.rfind("{"):log.rfind("}") + 1])
    music = args.work / "music.wav"
    normalization = "loudnorm=I=-16:TP=-1.5:LRA=11:linear=true:" + ":".join(f"{target}={loudness[source]}" for target, source in [("measured_I", "input_i"), ("measured_LRA", "input_lra"), ("measured_TP", "input_tp"), ("measured_thresh", "input_thresh"), ("offset", "target_offset")])
    run(["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-ss", str(plan["music"]["start"]), "-t", str(plan["duration"]), "-i", str(args.music), "-af", normalization + f",afade=t=in:d=0.4,afade=t=out:st={plan['duration']-2.8}:d=2.8", "-ar", "48000", "-ac", "2", "-c:a", "pcm_s24le", str(music)], args.work / "logs/music.log")
    output = args.out / "MorphForge-Promo-zh-CN-1080p60.mp4"
    multilingual = args.out / "MorphForge-Promo-Multisub-1080p60.mp4"
    command = ["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-i", str(picture), "-i", str(music)]
    for language in ["ja", "en"]:
        command += ["-i", str(args.out / f"MorphForge-Promo.{language}.ass")]
    command += ["-map", "0:v:0", "-map", "1:a:0", "-map", "2:s:0", "-map", "3:s:0", "-c:v", "copy", "-c:a", "aac", "-b:a", "224k", "-c:s", "mov_text", "-metadata:s:s:0", "language=jpn", "-metadata:s:s:0", "title=日本語", "-metadata:s:s:1", "language=eng", "-metadata:s:s:1", "title=English", "-disposition:s:0", "0", "-disposition:s:1", "0", "-metadata", "title=MorphForge — 把想法带进 VRChat", "-metadata", "comment=Music: Cipher by Kevin MacLeod (incompetech.com), CC BY 4.0. Edited excerpt.", "-t", str(plan["duration"]), "-movflags", "+faststart", str(multilingual)]
    run(command, args.work / "logs/final-mux.log")
    # MP4 may enable its first subtitle even when every subtitle disposition was
    # cleared. Preserve a plain Chinese master for predictable upload/playback.
    run(["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-i", str(multilingual), "-map", "0:v:0", "-map", "0:a:0", "-c", "copy", "-sn", "-movflags", "+faststart", str(output)], args.work / "logs/plain-master.log")
    for source, target in [(args.plan, args.out / "timeline.json"), (Path(__file__), args.out / "render_promo_video.py")]:
        if source.resolve() != target.resolve():
            shutil.copy2(source, target)
    assets = args.out / "assets"
    assets.mkdir(exist_ok=True)
    still_assets = {
        "workbench": ["docs/releases/0.27.0/media/leviathan-lantern-editor.png"],
        "drawing": ["docs/optimization-20261023/drawing-thin.png", "docs/optimization-20261023/drawing-thick.png"],
        "publish": ["docs/default-motion/native/main-publish-with-motion.png"],
    }
    for relative in sorted({path for shot in plan["shots"] for path in still_assets.get(shot["kind"], [])}):
        source = asset(relative)
        if source.resolve() != (assets / source.name).resolve():
            shutil.copy2(source, assets / source.name)
    if args.music.resolve() != (assets / "Cipher.mp3").resolve():
        shutil.copy2(args.music, assets / "Cipher.mp3")
    for name in ["NotoSansCJK-Regular.ttc", "NotoSansCJK-Bold.ttc"]:
        source = ROOT / "assets" / name
        if not source.exists():
            source = Path("/usr/share/fonts/noto-cjk") / name
        if source.exists() and source.resolve() != (assets / name).resolve():
            shutil.copy2(source, assets / name)
    font_license = ROOT / "assets/OFL-NotoSansCJK.txt"
    if not font_license.exists():
        font_license = Path("/usr/share/licenses/noto-fonts-cjk/LICENSE")
    if font_license.exists() and font_license.resolve() != (assets / "OFL-NotoSansCJK.txt").resolve():
        shutil.copy2(font_license, assets / "OFL-NotoSansCJK.txt")
    typography = {code: {weight: {"file": Path(font_face(code, weight == "bold")[0]).name, "faceIndex": font_face(code, weight == "bold")[1], "family": font_face(code, weight == "bold")[2], "style": font_face(code, weight == "bold")[3]} for weight in ["regular", "bold"]} for code in ["zh-CN", "ja", "en"]}
    (args.out / "typography.json").write_text(json.dumps(typography, ensure_ascii=False, indent=2) + "\n")
    japanese = args.out / "MorphForge-Promo-ja-1080p60.mp4"
    subtitle_filter = f"subtitles=filename='{filter_path(args.out / 'MorphForge-Promo.ja.ass')}':fontsdir='{filter_path(assets)}'"
    run(["ffmpeg", "-hide_banner", "-loglevel", "info", "-y", "-filter_threads", "2", "-threads", "4", "-i", str(output), "-map", "0:v:0", "-map", "0:a:0", "-vf", subtitle_filter, "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-threads", "4", "-pix_fmt", "yuv420p", "-r", "60", "-c:a", "copy", "-sn", "-movflags", "+faststart", str(japanese)], args.work / "logs/japanese-font-burn.log")
    (args.out / "output-probe.json").write_text(json.dumps(probe(output), ensure_ascii=False, indent=2) + "\n")
    (args.out / "multisub-probe.json").write_text(json.dumps(probe(multilingual), ensure_ascii=False, indent=2) + "\n")
    (args.out / "japanese-probe.json").write_text(json.dumps(probe(japanese), ensure_ascii=False, indent=2) + "\n")
    print(output, flush=True)


if __name__ == "__main__":
    main()
