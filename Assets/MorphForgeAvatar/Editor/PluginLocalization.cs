using System;
using System.Collections.Generic;
using UnityEditor;
using UnityEngine;

namespace MorphForgeAvatar.Editor {
    public static class PluginLocalization {
        public static readonly string[] Codes = { "zh-CN", "ja", "en" };
        public static readonly string[] Names = { "简体中文", "日本語", "English" };
        const string PreferenceKey = "MorphForgeAvatar.InterfaceLanguage";
        public static string SelectedLanguage {
            get {
                var value = EditorPrefs.GetString(PreferenceKey, "");
                if (Array.IndexOf(Codes, value) >= 0) return value;
                return Application.systemLanguage == SystemLanguage.Japanese ? "ja" :
                    Application.systemLanguage == SystemLanguage.Chinese || Application.systemLanguage == SystemLanguage.ChineseSimplified || Application.systemLanguage == SystemLanguage.ChineseTraditional ? "zh-CN" : "en";
            }
            set {
                if (Array.IndexOf(Codes, value) < 0) throw new ArgumentException("Unsupported interface language");
                EditorPrefs.SetString(PreferenceKey, value);
            }
        }
        public static string Text(string source) => Translate(source, SelectedLanguage);
        public static string Translate(string source, string language) {
            if (source == null) return null;
            if (language != "zh-CN" && Messages.TryGetValue(source, out var values)) return values[language == "ja" ? 1 : 0];
            return source;
        }
        public static string MenuText(string label, string language) {
            // Canonicalize only known plugin labels. Custom names are retained.
            if (!Messages.ContainsKey(label)) foreach (var pair in Messages) {
                if (pair.Value[0] == label || pair.Value[1] == label) { label = pair.Key; break; }
            }
            return Translate(label, language);
        }
        static readonly Dictionary<string, string[]> Messages = new Dictionary<string, string[]> {
            { "缩放比例", new [] { "Scale", "拡大縮小" } },
            { "连续缩放（上大下小）", new [] { "Continuous scale", "連続拡大縮小" } },
            { "100%（标准）", new [] { "100% (standard)", "100%（標準）" } },
            { "MorphForge：合并后缺少 Expression Parameters。", new [] { "MorphForge: Expression Parameters missing after merge.", "MorphForge：統合後にExpression Parametersがありません。" } },
            { "MorphForge：构建预算检查失败（", new [] { "MorphForge: build budget check failed (", "MorphForge：ビルド予算の検証に失敗（" } },
            { "OSC 数据必须为直接同步 Int，不能加入压缩菜单：", new [] { "OSC data must be directly synced Int parameters, not compressed: ", "OSCデータは圧縮せずIntで直接同期する必要があります：" } },
            { "Tools/MorphForge Avatar/安装与检查", new [] { "Tools/MorphForge Avatar/Setup", "Tools/MorphForge Avatar/設定" } },
            { "。可将该 Prefab 拖到 Avatar 下；也可选择它后使用“安装所选变体”。", new [] { ". Drag this Prefab under the Avatar, or select it and use “Install selected variant”.", "。このPrefabをAvatarの下に配置するか、選択して「選択したバリアントをインストール」を使ってください。" } },
            { "。请在 SDK 构建后查看最终预算。", new [] { ". Check the final budget after the SDK build.", "。SDKビルド後に最終予算を確認してください。" } },
            { "一键安装到所选 Avatar", new [] { "Install on selected Avatar", "選択Avatarにインストール" } },
            { "人物参数已使用 MorphForge 保留名称 MF6_* / MF_*，请先处理命名冲突。", new [] { "Avatar parameters use reserved MorphForge names MF6_* / MF_*. Resolve the naming conflicts first.", "AvatarパラメーターがMorphForgeの予約名MF6_* / MF_*を使用しています。名前の競合を先に解決してください。" } },
            { "以 SDK 构建检查报告为准", new [] { "Use the final SDK build report", "SDKビルドの最終レポートで確認" } },
            { "位置", new [] { "Position", "位置" } },
            { "体素缩放", new [] { "Voxel scale", "ボクセル倍率" } },
            { "保持水平", new [] { "Keep level", "水平を維持" } },
            { "允许本人抓握", new [] { "Allow owner grabbing", "本人のつかみ操作を許可" } },
            { "内置原创方块工坊材质，不包含 Mojang 或人物资产。自定义目录由 Desktop 导出，选择包含 catalog.json 与 artifacts 的文件夹；插件会生成新的独立变体，不覆盖已安装模块。纹理变化需要重新上传 Avatar，实时纯色/程序材质不需要。", new [] { "Includes original block-workshop materials, without Mojang or character assets. Export a custom catalog from Desktop and select its catalog.json/artifacts folder. Creates an independent variant without replacing installed modules. Texture changes require re-upload; live color/procedural materials do not.", "オリジナルのブロック工房材質を収録し、Mojangやキャラクターの素材は含みません。Desktopからカスタムカタログを出力し、catalog.jsonとartifactsを含むフォルダーを選択します。既存モジュールを上書きせず独立バリアントを作成します。テクスチャ変更は再アップロードが必要ですが、リアルタイム色・手続き型材質は不要です。" } },
            { "几何固定使用 240 bit。定位默认使用 13 bit；光球可见性与允许本人抓握独立，本地控制。VRCFury 可压缩菜单中的定位/服装等低频参数，不能压缩逐帧面捕和 OSC 数据。预算超出会阻止构建，不会擅自取消任何其他参数的同步。", new [] { "Geometry uses 240 bits; default placement uses 13 bits. Anchor visibility and owner grabbing are independent local controls. VRCFury compresses low-frequency placement/clothing menu parameters, not per-frame face tracking or OSC data. Exceeding the budget blocks the build without disabling other parameter synchronization.", "形状は240bit、標準配置は13bitを使用します。アンカー表示と本人のつかみ操作は独立したローカル設定です。VRCFuryは配置・衣装など低頻度のメニューパラメーターを圧縮し、毎フレームの表情追跡やOSCデータは圧縮しません。予算超過時は他の同期を無効にせずビルドを停止します。" } },
            { "取景缩放", new [] { "View zoom", "表示ズーム" } },
            { "右上", new [] { "Top right", "右上" } },
            { "右下", new [] { "Bottom right", "右下" } },
            { "右侧", new [] { "Right", "右側" } },
            { "右手跟随（菜单）", new [] { "Follow right hand", "右手に追従" } },
            { "启用 VRCFury 参数压缩桥", new [] { "Enable VRCFury parameter compression bridge", "VRCFuryパラメーター圧縮ブリッジを有効化" } },
            { "固定在世界", new [] { "Fix in world", "ワールドに固定" } },
            { "在 VCC Avatar 工程安装 Modular Avatar 后，选择人物并安装。上传后由 MorphForge Desktop 经 OSC 驱动。PC / Windows；几何总容量16,384个Box或三角面，另有4个128槽中日英点阵文本框，以及3组×4关键帧的本地动画。可选本人头边预览为实验功能，默认关闭。", new [] { "Install Modular Avatar in a VCC Avatar project, then select your character and install. MorphForge Desktop drives it through OSC after upload. PC/Windows; capacity 16,384 boxes or triangles, four 128-slot Chinese/Japanese/English text panels and three groups of four local-animation keyframes. Experimental owner preview is off by default.", "VCCのAvatarプロジェクトにModular Avatarを導入し、キャラクターを選択してインストールしてください。アップロード後、MorphForge DesktopがOSCで制御します。PC / Windows対応。Box・三角面の合計容量16,384個、128文字枠の中日英テキストパネル4枚、4キーフレームのローカルアニメーション3グループを搭載。実験的な本人プレビューは標準でオフです。" } },
            { "基础 Prefab 缺失。", new [] { "Base Prefab missing.", "基本Prefabがありません。" } },
            { "复制 Decoder 失败。", new [] { "Failed to copy Decoder.", "Decoderの複製に失敗しました。" } },
            { "安装 Project 中所选的 MorphForge 变体", new [] { "Install selected MorphForge variant from Project", "Projectで選択したMorphForgeバリアントをインストール" } },
            { "安装菜单语言", new [] { "Installed menu language", "インストール済みメニューの言語" } },
            { "安装资源不完整。", new [] { "Installation assets are incomplete.", "インストール用リソースが不足しています。" } },
            { "定位设置", new [] { "Placement settings", "配置設定" } },
            { "导入 Desktop 材质目录并生成安装变体…", new [] { "Import Desktop catalog and create installable variant…", "Desktopカタログを読み込み、インストール用バリアントを作成…" } },
            { "导出内置 Desktop 配套材质目录 ZIP…", new [] { "Export bundled Desktop catalog ZIP…", "内蔵DesktopカタログZIPを出力…" } },
            { "导出材质目录", new [] { "Export material catalog", "材質カタログを出力" } },
            { "将当前语言应用到所选 Avatar 菜单", new [] { "Apply current language to selected Avatar menus", "現在の言語を選択Avatarのメニューに適用" } },
            { "左上", new [] { "Top left", "左上" } },
            { "左下", new [] { "Bottom left", "左下" } },
            { "左侧", new [] { "Left", "左側" } },
            { "左手跟随（菜单）", new [] { "Follow left hand", "左手に追従" } },
            { "已安装所选材质变体。", new [] { "Selected material variant installed.", "選択した材質バリアントをインストールしました。" } },
            { "已安装：", new [] { "Installed: ", "インストール済み：" } },
            { "已导出 ", new [] { "Exported ", "出力済み " } },
            { "已检测", new [] { "Detected", "検出済み" } },
            { "已检测（可选）", new [] { "Detected (optional)", "検出済み（任意）" } },
            { "已生成 ", new [] { "Created ", "作成済み " } },
            { "当前人物参数（合并前）", new [] { "Current parameters (before merge)", "現在のパラメーター（統合前）" } },
            { "所选语言用于插件界面及所选 Avatar 的 MorphForge 菜单。更改后重新上传 Avatar，VRChat 菜单才会更新。", new [] { "The selected language controls this window and the selected Avatar’s MorphForge menus. Re-upload the Avatar to update the menus in VRChat.", "選択した言語をこの画面と選択AvatarのMorphForgeメニューに使用します。VRChatに反映するにはAvatarを再アップロードしてください。" } },
            { "手部跟随", new [] { "Hand following", "手への追従" } },
            { "斜视取景", new [] { "Oblique view", "斜め視点" } },
            { "无法创建独立材质目录。", new [] { "Cannot create independent material catalog.", "独立した材質カタログを作成できません。" } },
            { "显示定位球", new [] { "Show anchor", "アンカーを表示" } },
            { "显示本体", new [] { "Show geometry", "本体を表示" } },
            { "显示预览", new [] { "Show preview", "プレビューを表示" } },
            { "最终预算", new [] { "Final budget", "最終予算" } },
            { "未安装 VRCFury。", new [] { "VRCFury is not installed.", "VRCFuryがインストールされていません。" } },
            { "未安装（可选）", new [] { "Not installed (optional)", "未導入（任意）" } },
            { "未安装（必需）", new [] { "Not installed (required)", "未導入（必須）" } },
            { "本人预览", new [] { "Owner preview", "本人プレビュー" } },
            { "材质目录格式不支持。", new [] { "Unsupported material catalog format.", "未対応の材質カタログ形式です。" } },
            { "检查在 VRCFury 压缩之后执行；不会关闭其他参数的同步。", new [] { "Checked after VRCFury compression; other parameter synchronization is unchanged.", "VRCFury圧縮後に検証します。他のパラメーター同期は変更しません。" } },
            { "模块已移除，可通过 Undo 恢复。", new [] { "Module removed. Undo can restore it.", "モジュールを削除しました。元に戻せます。" } },
            { "此 Avatar 已安装 MorphForge，请先移除旧模块（支持 Undo）。", new [] { "MorphForge is already installed on this Avatar. Remove the old module first (Undo supported).", "このAvatarにはMorphForgeが導入済みです。先に旧モジュールを削除してください（元に戻せます）。" } },
            { "此 Avatar 已有 MF6 显示模块，请先移除旧模块，避免重复接收同一 OSC 命令。", new [] { "This Avatar already has an MF6 display module. Remove it first to avoid receiving each OSC command twice.", "このAvatarにはMF6表示モジュールがあります。同じOSCコマンドの重複受信を避けるため先に削除してください。" } },
            { "目标 Avatar", new [] { "Target Avatar", "対象Avatar" } },
            { "直接相加估计（压缩前）", new [] { "Direct sum estimate (before compression)", "単純加算の推定（圧縮前）" } },
            { "移到面前", new [] { "Move in front", "目の前に移動" } },
            { "移除所选 Avatar 的 MorphForge 模块", new [] { "Remove MorphForge from selected Avatar", "選択AvatarのMorphForgeを削除" } },
            { "纹理必须来自目录 artifacts。", new [] { "Textures must come from the catalog’s artifacts folder.", "テクスチャはカタログのartifactsフォルダー内に置いてください。" } },
            { "菜单语言已更新；其他模块、参数和材质保持不变。", new [] { "Menu language updated. Other modules, parameters and materials are unchanged.", "メニュー言語を更新しました。他のモジュール・パラメーター・材質は保持しています。" } },
            { "语言 / Language / 言語", new [] { "Language / 语言 / 言語", "言語 / Language / 语言" } },
            { "请减少其他同步参数，或安装 VRCFury 压缩低频菜单参数。详见 Logs/MorphForge/avatar-budget.json。", new [] { "Reduce other synced parameters or install VRCFury to compress low-frequency menu parameters. See Logs/MorphForge/avatar-budget.json.", "他の同期パラメーターを減らすか、VRCFuryで低頻度メニューパラメーターを圧縮してください。詳細：Logs/MorphForge/avatar-budget.json。" } },
            { "请在 Project 中选择 MorphForge 模块 Prefab。", new [] { "Select a MorphForge module Prefab in Project.", "ProjectでMorphForgeモジュールPrefabを選択してください。" } },
            { "请安装到场景中的 Avatar 实例。", new [] { "Install on an Avatar instance in a scene.", "シーン内のAvatarインスタンスにインストールしてください。" } },
            { "贴图库", new [] { "Texture catalog", "テクスチャカタログ" } },
            { "选择包含 catalog.json 的目录", new [] { "Select the folder containing catalog.json", "catalog.jsonを含むフォルダーを選択" } },
            { "需要 Edit Mode、Avatar Descriptor 和 Modular Avatar。", new [] { "Edit Mode, Avatar Descriptor and Modular Avatar are required.", "Edit Mode・Avatar Descriptor・Modular Avatarが必要です。" } },
            { "需要空闲 Edit Mode。", new [] { "Idle Edit Mode required.", "操作待機中のEdit Modeが必要です。" } },
            { "非法纹理文件名。", new [] { "Invalid texture filename.", "テクスチャファイル名が無効です。" } },
            { "面板大小", new [] { "Panel size", "パネルサイズ" } },
        };
    }
}
