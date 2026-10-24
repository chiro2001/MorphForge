# MorphForge | Voxels, geometry and captions for VRChat

A Windows creative workbench with an avatar installation plugin.
Edit voxels, place points and lines, and send models, text and group animation to your own avatar through OSC. Manual editing and built-in examples need no API key. Connect your own LLM API for optional AI creation and subtitle translation.

[Features]
Voxels, triangles, points, adjustable line thickness and quick demonstration shapes.
Chinese, Japanese and English bitmap text with half/full-width layout, adjustable size, position and line spacing.
Live captions with the original text retained; optional Chinese/Japanese/English translation. New panels default to four lines: two original and two translated.
Group animation: translation, rotation, uniform scale, looping/one-shot and pause/resume.
Desktop follows the system language and switches instantly. Unity offers a language choice for its interface and the installed avatar menu.
544 standard materials and editable built-in sample works.

[Included]
MorphForge Desktop 0.27.0 Windows x64 installer.
MorphForge Avatar 1.4.0 Unitypackage and matching public material catalog ZIP.
Chinese, Japanese and English quick-start guides, license notices and SHA256 hashes.
No character avatar, SDK, Modular Avatar, NDMF, VRCFury, API key or cloud credits are included.

[Before using]
Windows PC/PCVR. Standalone Quest/Android is unverified.
Tested: Unity 2022.3.22f1, SDK 3.10.5, Modular Avatar 1.18.7 and NDMF 1.14.8; optional VRCFury 1.1430.0.
The default module uses 253 synchronized bits, potentially conflicting with existing clothes or face tracking. VRCFury cannot compress the OSC data. Over-budget builds are blocked.
This experimental module assumes a Very Poor avatar performance rank. No real-client FPS or universal avatar compatibility is promised.

[Limits]
256³ logical space; configured capacity of 16,384 geometry objects. Full-capacity 16K lossless network endurance testing was not repeated for this version.
Four text panels with 128 UTF-16 slots each, including line breaks; captions share this limit.
Avatar animation supports three groups with four keyframes each, a subset of full editor animation.
Completion means local UDP transmission has finished; there is no remote execution ACK. Late joiners or missed data require replay.
New image textures and avatar menu language changes require avatar re-upload. No general physical collision.
Owner preview is experimental, default off, and adds a camera render cost.
Product images are actual Desktop interface/render captures, not VRChat client captures.

[AI and speech]
AI creation and subtitle translation use your own compatible API; provider fees are separate. A small Silero VAD is included. SenseVoice/Zipformer recognition weights are separate downloads; Tencent Cloud needs your own credentials. Recording and external services are enabled by the user.

[Licensing and production]
The program's Cargo metadata declares MIT. Original textures carry their own MIT notice; fonts use SIL OFL 1.1 and libraries retain their original terms.
Included whale reference images and the whale-maid derivative sample use CC BY-NC-SA 4.0 (original character: 上善无形; derivative references: ZipZipPipe). This bundle does not grant unrestricted commercial use of every asset. See Licenses for details.
Implementation, some samples and documentation used AI assistance with human direction and review. This is not an official VRChat or Unity product.

[Publisher confirmation required]
The current price is a 0 JPY draft. Confirm the final price and product terms and add a support contact before publication. Resolve these items before removing this paragraph.
