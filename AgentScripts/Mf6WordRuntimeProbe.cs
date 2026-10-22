using System;
using System.Linq;
using Newtonsoft.Json.Linq;
using UnityEngine;
using UnityEditor;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class Mf6WordRuntimeProbe {
    public static string Motion() {
        var m=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;
        var r=m.Avatar.GetComponentsInChildren<ParticleSystemRenderer>(true).Single(x=>x.name=="Store"&&x.transform.parent.name=="Pool0");
        var block=new MaterialPropertyBlock();r.GetPropertyBlock(block);var values=new JObject();
        foreach(string key in new[]{"Enabled","Bank","Base","Loop","Elapsed","B1_Count","B1_Interp","B1_K0T","B1_K1T","B1_K1QX","B1_K1QY","B1_K1QZ","B1_K1QW","B1_K1S"}) {
            string prop=key.StartsWith("B")&&key.Length>2&&char.IsDigit(key[1])?"_MF_M1"+key:"_MF_M1_"+key;
            values[prop]=block.HasFloat(prop)?block.GetFloat(prop):r.sharedMaterial.GetFloat(prop);
        }
        foreach(string key in new[]{"Enabled","Bank","B1_Ready","B1_Version","B1_Count","B1_C02","B1_C34","B1_C44","B1_C46","B1_C48","B1_C50"})values["MF6_M1_"+key]=m.GetParam("MF6_M1_"+key)?.FloatValue();
        return values.ToString();
    }
    public static async System.Threading.Tasks.Task<string> Trace() {
        var module=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;
        var data=JObject.Parse(System.IO.File.ReadAllText("E:/tmp/morphforge/text-fixtures.json"));var rows=new JArray();
        using(var udp=new System.Net.Sockets.UdpClient())foreach(var frame in data["frames"]) {
            var bytes=frame["packet"].Select(v=>(byte)(int)v).ToArray();udp.Send(bytes,bytes.Length,"127.0.0.1",9012);await System.Threading.Tasks.Task.Delay(350);
            var values=new JObject();foreach(string name in new[]{"MF6_Stage","MF6_Q00","MF6_Q01","MF6_Q02","MF6_T03","MF6_T04","MF6_WN03","MF6_WC03","MF6_Text0_B000","MF6_Text0_B024","MF6_Text0_B256","MF6_Text0_Page0"})values[name]=module.GetParam(name)?.FloatValue();
            rows.Add(values);
        }
        return rows.ToString();
    }
    public static string Inspect() {
        var module = UnityEngine.Object.FindObjectsOfType<GestureManager>().Single().Module as ModuleVrc3;
        if (module == null) throw new Exception("Gesture Manager Play Mode required.");
        var flags = System.Reflection.BindingFlags.Instance | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic;
        var layers = (System.Collections.IDictionary)module.GetType().GetField("_layers", flags).GetValue(module);
        var key = layers.Keys.Cast<object>().Single(k => k.ToString() == "FX");
        var playable = (UnityEngine.Animations.AnimatorControllerPlayable)layers[key].GetType().GetField("Playable", flags).GetValue(layers[key]);
        var names = new[] { "MF6_T00", "MF6_T01", "MF6_T02", "MF6_T03", "MF6_TextOne", "MF6_MOne", "MF6_Text0_Count", "MF6_Text0_Page0", "MF6_Text0_B000", "MF6_Text0_B002", "MF6_Text0_B256" };
        var values = new JArray(names.Select(n => (JToken)new JObject { ["name"] = n,
            ["module"] = module.GetParam(n)?.FloatValue(), ["playable"] = playable.GetFloat(n) }));
        var controller = module.Avatar.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>().baseAnimationLayers.Single(l => l.type == VRC.SDK3.Avatars.Components.VRCAvatarDescriptor.AnimLayerType.FX).animatorController;
        var curves = new JArray(controller.animationClips.Where(c => c.name.StartsWith("Cache MF6_Text0_B000/")).Select(c => (JToken)new JObject {
            ["clip"] = c.name, ["bindings"] = new JArray(AnimationUtility.GetCurveBindings(c).Select(b => (JToken)new JObject { ["path"] = b.path, ["type"] = b.type.FullName, ["property"] = b.propertyName })) }));
        return new JObject { ["values"] = values, ["curves"] = curves,
            ["layers"] = new JArray(Enumerable.Range(0, playable.GetLayerCount()).Where(i => playable.GetLayerName(i).Contains("word cache")).Select(i => (JToken)new JObject { ["name"] = playable.GetLayerName(i), ["weight"] = playable.GetLayerWeight(i) })) }.ToString();
    }
}
