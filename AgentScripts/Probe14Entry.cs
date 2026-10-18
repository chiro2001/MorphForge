using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEngine;

public static class Probe14Entry
{
    public static string Build()
    {
        return MorphForge.Editor.Probe14Build.Build();
    }

    public static string ValidateExport()
    {
        if (EditorApplication.isPlaying) throw new InvalidOperationException("Validate export in Edit Mode.");
        var source=AssetDatabase.LoadAssetAtPath<GameObject>(MorphForge.Editor.Probe14Build.Root+"/ProbeAvatar.prefab");
        var clone=UnityEngine.Object.Instantiate(source);
        clone.hideFlags=HideFlags.HideAndDontSave;
        try
        {
            nadena.dev.ndmf.AvatarProcessor.ProcessAvatar(clone);
            var descriptor=clone.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>();
            var allParameters=descriptor.expressionParameters.parameters;
            var parameters=descriptor.expressionParameters.parameters.Where(p=>p.name.StartsWith("MFProbe_")).ToArray();
            int existingSyncedBits=allParameters.Where(p=>p.networkSynced).Sum(p=>p.valueType.ToString()=="Bool"?1:8);
            var names=parameters.Select(p=>p.name).ToArray();
            int missing=clone.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));
            bool pass=parameters.Length==75 && parameters.All(p=>!p.networkSynced) && missing==0 &&
                Enumerable.Range(0,72).All(i=>names.Contains("MFProbe_V"+i.ToString("D2")));
            var json=Newtonsoft.Json.JsonConvert.SerializeObject(new {
                success=pass, scope="Editor NDMF/MA processing, not SDK upload or VRChat runtime",parameterCount=parameters.Length,
                syncedProbeParameters=parameters.Count(p=>p.networkSynced),missingScripts=missing,parameterNames=names,
                totalAvatarParameters=allParameters.Length,avatarSyncedBits=existingSyncedBits,
                remainingSyncedBits=256-existingSyncedBits,
                probeBitsIfAllSynced=parameters.Sum(p=>p.valueType.ToString()=="Bool"?1:8)
            },Newtonsoft.Json.Formatting.Indented);
            Directory.CreateDirectory("Docs/Probe14");File.WriteAllText("Docs/Probe14/export.json",json);
            if(!pass)throw new InvalidOperationException(json);
            return json;
        }
        finally {UnityEngine.Object.DestroyImmediate(clone);}
    }

    public static string FrameLiveDemo()
    {
        if (!EditorApplication.isPlaying) throw new InvalidOperationException("Enter Play Mode first.");
        var manager=UnityEngine.Object.FindObjectsOfType<BlackStartX.GestureManager.GestureManager>()
            .First(g=>g.name=="MorphForge Gesture Manager");
        var module=manager.Module as BlackStartX.GestureManager.Editor.Modules.Vrc3.ModuleVrc3;
        if(module==null)throw new InvalidOperationException("Start probe OSC first.");
        var root=module.Avatar.transform.Find(MorphForge.Editor.Probe14Build.ModuleName);
        foreach(var renderer in root.GetComponentsInChildren<MeshRenderer>())renderer.gameObject.layer=31;
        var camera=UnityEngine.Object.FindObjectsOfType<Camera>().First(c=>c.name=="Probe Camera");
        var center=root.TransformPoint(new Vector3(-.25f,0,0));
        camera.transform.position=center+root.TransformDirection(new Vector3(1.2f,.9f,4f)).normalized*3;
        camera.transform.LookAt(center);
        camera.orthographic=true;camera.orthographicSize=root.lossyScale.x*1.45f;
        camera.cullingMask=1<<31;camera.clearFlags=CameraClearFlags.SolidColor;
        camera.backgroundColor=new Color(.035f,.05f,.08f);
        var type=typeof(UnityEditor.Editor).Assembly.GetType("UnityEditor.GameView");
        var window=EditorWindow.GetWindow(type);window.Show();window.Focus();
        return "Game view focused on the two probes; runtime-only framing.";
    }
}
