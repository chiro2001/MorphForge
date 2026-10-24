# MorphForge 0.27.0 quick start

Includes MorphForge Avatar 1.4.0. Prepared 2026-10-25.

MorphForge is a Windows voxel and geometry workbench with manual editing, Recipes, and optional assistance from your own LLM API. After installing the module on your own Humanoid avatar and uploading it, use OSC to display geometry, points, lines, text and group animation in VRChat PC/PCVR. A character avatar is not included.

## Requirements

- Desktop: Windows x64, WebView2 (Microsoft Edge), and working WebGL graphics. No macOS/Linux installer is included.
- Tested combination: Unity 2022.3.22f1, VRChat SDK 3.10.5, Modular Avatar 1.18.7, NDMF 1.14.8; optional VRCFury 1.1430.0. Install these dependencies separately through VCC.
- PC/PCVR only; standalone Quest/Android is unverified. Expect a Very Poor avatar performance rank. No real-client FPS is promised.
- The standard module costs **253 synchronized bits**: 240 geometry + 13 placement. Back up your avatar and check its remaining budget. VRCFury can compress suitable low-frequency menu parameters, not OSC data or continuous face tracking. An over-budget build is blocked.

## Install Desktop

1. Run `Windows/MorphForge Desktop_0.27.0_x64-setup.exe` and follow the installer.
2. In Settings, choose System / 简体中文 / 日本語 / English. Language changes apply immediately.
3. Load a built-in work such as “会转动的风车” (rotating windmill). Manual editing and built-in examples need no API key.
4. For AI creation or subtitle translation, configure your own compatible Chat/Responses API. No key or service credit is included; provider charges are separate. UI language, prompt language, speech recognition and translation language are independent.

## Install the avatar module

1. Back up your scene. Prepare a VCC Humanoid avatar project with SDK, Modular Avatar and its NDMF dependency.
2. Import `Unity/MorphForgeAvatar-1.4.0.unitypackage`.
3. Open `Tools > MorphForge Avatar > Setup - 安装 - 設定`, select the avatar and language, then install. Install VRCFury first if using the optional compression bridge.
4. Remove any older MF6 module before installing to avoid duplicate receivers. Keep the shipped controllers' required Write Defaults On settings. Save the scene and build/upload normally with the SDK.
5. The language controls both the plugin interface and this avatar's menu. Existing modules can also receive the selected language. **Re-upload the avatar to change its menu language in VRChat.**
6. If Desktop does not recognize the matching 544-material public catalog, import `Unity/MorphForge-public-catalog.zip` and detect the avatar again. This catalog matches the standard module; custom texture variants need their own matching export.

## First publication and animation

1. Wear the uploaded avatar in VRChat and enable OSC. Detect it from Desktop's connection page; check capability version **9** and the material catalog. If detection fails, check OSC ports, firewall rules and stale avatar caches.
2. Load the rotating windmill sample and verify the model and rotation in the editor.
3. A single saved animation is selected automatically. With multiple clips, select the intended clip in the main publication area. “Publish current version” publishes the model and selected animation together. Static mode sends only the model.
4. Wait for every transmission stage, then check continuous blade rotation in VRChat. 100% means the local UDP sender has finished. **There is no remote execution ACK**, so this is not proof that every player received all content.
5. Use replay/loop replay for late joiners or missed data. Replay uses the latest model, text and animation definitions. Large works take time to send; instant delivery and exact cross-client phase alignment are not promised.

## Points, lines, text and captions

Specify point diameter or line thickness and both endpoints directly. Dedicated commands feed the avatar's native particle/shader renderer, sharing geometry capacity without transmitting vertices every frame. Diameter is a spatial size, so apparent thickness changes with viewing distance.

There are four text panels, each with 128 UTF-16 slots including line breaks. Chinese, Japanese and English bitmap glyphs support half/full-width layout, adjustable size, position, rotation, line spacing, color and bounds. Characters outside the BMP may show missing-glyph markers. Desktop performs layout; the avatar displays the received characters and layout.

New live-caption panels default to four fixed-size lines: up to the latest two original lines plus two translated lines. Optional translation targets Chinese, Japanese or English and retains the original text. Translation is disabled by default and uses your own LLM API. Existing panels keep their settings; adjust maximum lines and overflow if needed. Live captions and normal text share the four-panel limit and can appear with voxels and animation.

A small Silero VAD model is included. SenseVoice/Zipformer recognition weights require separate downloads; Tencent Cloud requires your own credentials. Users explicitly enable recording, cloud recognition and translation. No cloud credits are included.

## Limits and performance

- Logical space: 256³, X/Z -128…127 and Y 0…255. Avatar configured capacity is 16,384 geometry objects. Full-capacity 16K lossless network endurance testing was not repeated for this version.
- Avatar animation: 3 groups × 4 keyframes; translation, rotation, positive uniform scale; looping/one-shot, pause/resume/seek. Duration presets span 0.125…64 seconds. Full editor animation has a different feature set; follow the pre-publication checks.
- Solid colors, quantized PBR and procedural materials can be sent live. New image textures require a matching Unity variant and avatar re-upload.
- No general physical collision. Large scenes and overlapping transparency affect performance and appearance. Start with a small work. Editor previews do not guarantee the same appearance in a VRChat world.
- `MorphForge > Owner preview` is an experimental, default-off feature with an additional 512×384 camera render cost. VR comfort, stereo and mirrors need real-device confirmation.

## Updating and licensing

Back up your data and Unity scene before updating. Desktop data normally lives in `%LOCALAPPDATA%\MorphForge`. Do not send API keys, speech credentials or private works to the publisher.

Use `SHA256SUMS.txt` to verify files, `Changelog.txt` for changes and `Licenses/` for original attribution and license texts.

The program's Cargo metadata declares MIT, and original pixel textures have their own MIT notices. Included whale reference images and the whale-maid derivative example use **CC BY-NC-SA 4.0** (original character: 上善无形; derivative references: ZipZipPipe). Fonts use SIL OFL 1.1. Libraries retain their own terms. Do not describe the whole bundle as CC0 or all assets as commercially unrestricted. Final product terms, price and support contact await publisher confirmation; see `LICENSE.txt`.

Source: https://github.com/chiro2001/MorphForge . The included version is authoritative; a corresponding GitHub Release is not guaranteed to have been published. Implementation, some samples and documentation used AI assistance with human direction and review. Product images use actual editor captures and code-based layout.
