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


def header(image, shot, opaque=True):
    draw = ImageDraw.Draw(image)
    text(draw, (68, 35), "M", 34, GOLD, True)
    text(draw, (111, 43), "MorphForge", 26, WHITE)
    text(draw, (68, 97), shot["title"], 58, WHITE, True, max_width=1750)
    text(draw, (71, 183), shot["detail"], 32, MUTED, max_width=1750)
    text(draw, (1540, 45), shot["chapter"], 24, GOLD, max_width=320)
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
    draw = ImageDraw.Draw(image)
    # Keep the model unobstructed in the middle; leave the bottom subtitle zone free.
    for y in range(310):
        alpha = int(212 * max(0, 1 - y / 310) ** 1.5)
        draw.line((0, y, W, y), fill=(*BG, alpha))
    header(image, shot, False)
    face = font(25)
    label = shot["badge"]
    length = int(draw.textlength(label, font=face)) + 34
    draw.rounded_rectangle((W - length - 60, 884, W - 60, 936), radius=9, fill=(*BG, 215))
    text(draw, (W - length - 43, 892), label, 25, WHITE)
    draw.rectangle((68, 995, 1852, 998), fill=(43, 60, 73, 185))
    draw.rectangle((68, 995, 68 + int(1784 * (index + 1) / total), 998), fill=(*GOLD, 230))
    return image


def still_card(shot):
    image = canvas()
    draw = header(image, shot)
    media = "docs/releases/0.27.0/media/"
    if shot["kind"] == "workbench":
        card(image, asset(media + "workbench-zh-CN.png"), (66, 285, 1300, 650), label="MorphForge Desktop · 中文界面")
        for i, (title, detail) in enumerate([("说出想法", "文字 / 图片参考"), ("预览与调整", "模型 / 材质 / 文字"), ("发布到 Avatar", "选择动画后一起发送")]):
            y = 320 + i * 185
            text(draw, (1422, y), f"0{i+1}", 25, GOLD)
            text(draw, (1422, y + 40), title, 39, WHITE, True, 420)
            text(draw, (1422, y + 101), detail, 25, MUTED, max_width=420)
        text(draw, (73, 962), "AI 功能需自行配置兼容的大模型 API", 23, MUTED)
    elif shot["kind"] == "drawing":
        card(image, asset("docs/optimization-20261023/drawing-browser/drawing-preview.png"), (65, 302, 1115, 605), label="原生点线 · 真实软件面板", crop=(245, 90, 1255, 770))
        thin = asset("docs/optimization-20261023/drawing-thin.png")
        thick = asset("docs/optimization-20261023/drawing-thick.png")
        card(image, thin, (1265, 330, 255, 255), label="细")
        card(image, thick, (1570, 330, 255, 255), label="粗")
        text(draw, (1265, 635), "原生点 / 线粗细对比", 33, WHITE, True, 575)
        text(draw, (1265, 698), "Avatar 内渲染 · Unity 验收图", 25, MUTED, max_width=575)
        text(draw, (1265, 755), "直接指定端点，再调整大小", 29, MINT, max_width=575)
    elif shot["kind"] == "languages":
        for i, (code, label) in enumerate([("zh-CN", "简体中文"), ("ja", "日本語"), ("en", "English")]):
            x = 62 + i * 620
            card(image, asset(media + f"workbench-{code}.png"), (x, 350, 560, 380), label=label, label_language=code)
        text(draw, (132, 836), "Desktop 可即时切换", 36, WHITE, True)
        text(draw, (1030, 836), "Unity 所选语言用于安装菜单", 36, WHITE, True, 800)
        text(draw, (132, 904), "语言选择独立于字幕翻译设置", 27, MUTED)
        text(draw, (1030, 904), "安装后更新 VRChat 菜单需要重新上传 Avatar", 25, MUTED, max_width=800)
    elif shot["kind"] == "end":
        image = canvas()
        draw = ImageDraw.Draw(image)
        text(draw, (107, 120), "MorphForge", 135, WHITE, True)
        draw.rectangle((114, 319, 272, 325), fill=GOLD)
        text(draw, (111, 370), shot["title"], 68, WHITE, True, 1720)
        text(draw, (117, 496), "模型 / 动画 / 像素画 / 实时字幕 / 快速演示", 37, MINT, max_width=1680)
        text(draw, (117, 604), shot["detail"], 34, MUTED, max_width=1680)
        text(draw, (117, 701), "Windows · PC / PCVR · Avatar + OSC", 29, MUTED)
        text(draw, (117, 808), 'Music: “Cipher” — Kevin MacLeod (incompetech.com)', 29, WHITE)
        text(draw, (117, 860), "Creative Commons: By Attribution 4.0 · edited excerpt", 27, MUTED)
        text(draw, (117, 909), "https://creativecommons.org/licenses/by/4.0/", 26, MUTED)
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
            message = shot["title"] + "\n" + shot["detail"] if language == "zh-CN" else shot[language]
            start, end = position + .08, position + shot["duration"] - .08
            # Treat the two rapid opening shots as one readable caption.
            if shot["id"] == "01-hook-windmill":
                end = 3.12
            elif shot["id"] == "02-hook-props":
                position += shot["duration"]
                continue
            blocks.append(f"{len(blocks)+1}\n{timestamp(start)} --> {timestamp(end)}\n{message}\n")
            position += shot["duration"]
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
    if shot["kind"] in {"workbench", "drawing", "languages", "end"}:
        image = work / "graphics" / (shot["id"] + ".png")
        still_card(shot).save(image)
        frames = round(duration * 60)
        command += ["-i", str(image), "-vf", f"zoompan=z='1+0.012*on/{frames}':x='iw/2-iw/zoom/2':y='ih/2-ih/zoom/2':d={frames}:s=1920x1080:fps=60,setsar=1"]
        if shot["kind"] == "end":
            command[-1] += f",fade=t=out:st={duration-.4}:d=0.4"
    else:
        source = args.sources / shot["file"]
        if not source.exists():
            source = args.sources / args.plan_data["sources"][shot["file"]]
        speed = shot.get("speed", 1)
        vf = ""
        if shot.get("stabilize"):
            transform = work / "tree.trf"
            detect = ["ffmpeg", "-hide_banner", "-loglevel", "warning", "-y", "-threads", "4", "-ss", str(shot["start"]), "-t", str(duration * speed), "-i", str(source), "-vf", f"vidstabdetect=shakiness=6:accuracy=12:stepsize=8:fileformat=ascii:result='{filter_path(transform)}'", "-an", "-f", "null", "-"]
            run(detect, work / "logs" / "stabilize-detect.log")
            vf = f"vidstabtransform=input='{filter_path(transform)}':smoothing=24:optzoom=1:zoom=2:interpol=bicubic,"
        vf += f"setpts=(PTS-STARTPTS)/{speed},fps=60,scale=1920:1080,setsar=1"
        command += ["-ss", str(shot["start"]), "-t", str(duration * speed), "-i", str(source)]
        if shot["kind"] == "publish":
            background = canvas()
            draw = header(background, shot)
            card(background, asset("docs/default-motion/native/main-publish-with-motion.png"), (1295, 395, 550, 415), crop=(1282, 476, 1778, 702), label="真实发布面板截图")
            text(draw, (1306, 849), "模型 + 所选动画", 38, WHITE, True, 550)
            text(draw, (1306, 909), "一起发送到 Avatar", 30, MINT, max_width=550)
            text(draw, (84, 265), "VRChat 实机", 27, MINT)
            image = work / "graphics" / (shot["id"] + ".png")
            background.save(image)
            command += ["-loop", "1", "-framerate", "60", "-i", str(image), "-filter_complex", f"[0:v]{vf},scale=1160:652[v];[1:v][v]overlay=70:317:shortest=1[out]", "-map", "[out]"]
        else:
            overlay = video_overlay(shot, index, total)
            if shot["kind"] == "brand":
                overlay = Image.new("RGBA", (W, H), (0, 0, 0, 0))
                draw = ImageDraw.Draw(overlay)
                for x in range(W):
                    alpha = int(235 * max(0, 1 - x / 1750) ** .35)
                    draw.line((x, 0, x, H), fill=(*BG, alpha))
                text(draw, (106, 280), "MorphForge", 136, WHITE, True)
                draw.rectangle((114, 493, 300, 500), fill=(*GOLD, 255))
                text(draw, (113, 557), shot["detail"], 58, WHITE, True, 1750)
                text(draw, (118, 689), "Desktop + Avatar + OSC", 37, MINT)
            image = work / "graphics" / (shot["id"] + ".png")
            overlay.save(image)
            command += ["-loop", "1", "-framerate", "60", "-i", str(image), "-filter_complex", f"[0:v]{vf}[v];[v][1:v]overlay=0:0:shortest=1[out]", "-map", "[out]"]
    command += ["-t", str(duration), *encode_args(), str(destination)]
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
    for relative in ["docs/releases/0.27.0/media/workbench-zh-CN.png", "docs/releases/0.27.0/media/workbench-ja.png", "docs/releases/0.27.0/media/workbench-en.png", "docs/optimization-20261023/drawing-browser/drawing-preview.png", "docs/optimization-20261023/drawing-thin.png", "docs/optimization-20261023/drawing-thick.png", "docs/default-motion/native/main-publish-with-motion.png", "fonts/NotoSansSC-Regular.otf", "fonts/OFL-NotoSansSC.txt"]:
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
