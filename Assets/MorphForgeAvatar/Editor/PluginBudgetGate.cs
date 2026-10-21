using System;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Avatars.ScriptableObjects;
using VRC.SDKBase.Editor.BuildPipeline;

namespace MorphForgeAvatar.Editor {
    public sealed class PluginBudgetGate : IVRCSDKPreprocessAvatarCallback {
        public const string Marker = "MF_Plugin";
        public static string[] Geometry => Enumerable.Range(0, 29)
            .Select(i => "MF6_D" + i.ToString("D2")).Concat(new[] { "MF6_Ctl" }).ToArray();
        public int callbackOrder => int.MaxValue - 40; // VRCFury compressor is Max-100.
        public static bool IsPlugin(GameObject avatar) {
            var descriptor = avatar.GetComponent<VRCAvatarDescriptor>();
            return avatar.GetComponentInChildren<MorphForgeAvatarModule>(true) ||
                (descriptor && descriptor.expressionParameters &&
                 descriptor.expressionParameters.parameters.Any(p => p.name == Marker));
        }
        public bool OnPreprocessAvatar(GameObject avatar) {
            if (!IsPlugin(avatar)) return true;
            var parameters = avatar.GetComponent<VRCAvatarDescriptor>().expressionParameters;
            if (!parameters) throw new InvalidOperationException("MorphForge：合并后缺少 Expression Parameters。");
            var sync = parameters.parameters.Where(p => p.networkSynced && !string.IsNullOrEmpty(p.name)).ToArray();
            var invalid = Geometry.Where(name => sync.Count(p => p.name == name &&
                p.valueType == VRCExpressionParameters.ValueType.Int) != 1).ToArray();
            var marker = parameters.parameters.Where(p => p.name == Marker).ToArray();
            int cost = parameters.CalcTotalCost();
            bool success = invalid.Length == 0 && cost <= 256 && marker.Length == 1 && !marker[0].networkSynced;
            var report = new {
                success, totalBits = cost, remainingBits = 256 - cost,
                geometryBits = 240, invalidDirectParameters = invalid,
                parameters = parameters.parameters.Select(p => new { p.name, type = p.valueType.ToString(), p.networkSynced }).ToArray(),
                note = "检查在 VRCFury 压缩之后执行；不会关闭其他参数的同步。"
            };
            Directory.CreateDirectory("Logs/MorphForge");
            File.WriteAllText("Logs/MorphForge/avatar-budget.json", JsonConvert.SerializeObject(report, Formatting.Indented));
            if (!success) throw new InvalidOperationException(
                "MorphForge：构建预算检查失败（" + cost + "/256 bit）。" +
                (invalid.Length > 0 ? "OSC 数据必须为直接同步 Int，不能加入压缩菜单：" + string.Join(", ", invalid) + "。" : "") +
                "请减少其他同步参数，或安装 VRCFury 压缩低频菜单参数。详见 Logs/MorphForge/avatar-budget.json。");
            return true;
        }
    }
}
