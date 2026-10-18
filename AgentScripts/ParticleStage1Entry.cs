using UnityEditor;
using System;
using System.IO;
using System.Linq;
using UnityEngine;

public static class ParticleStage1Entry
{
    public static string ActiveScene(){return UnityEngine.SceneManagement.SceneManager.GetActiveScene().path;}
    public static string StartOsc(){return MorphForge.Editor.ParticleStage1Harness.StartOsc();}
    public static string StopOsc(){return MorphForge.Editor.ParticleStage1Harness.StopOsc();}
    public static string Capture(string label,string expectedPath,bool images){return MorphForge.Editor.ParticleStage1Harness.Capture(label,expectedPath,images);}
    public static string CaptureFaces(string label,string expectedPath){return MorphForge.Editor.ParticleStage1Harness.CaptureFaces(label,expectedPath);}
    public static string FinishOwnedTest()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Exit the owned test first.");
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(scene.path!=MorphForge.Editor.ParticleStage1Build.ScenePath)throw new InvalidOperationException("Only the generated Stage1 test scene may be restored.");
        scene=UnityEditor.SceneManagement.EditorSceneManager.OpenScene(scene.path);
        return Newtonsoft.Json.JsonConvert.SerializeObject(new{scene=scene.path,scene.isDirty,playMode=EditorApplication.isPlaying});
    }
    public static string Build(){return MorphForge.Editor.ParticleStage1Build.Build();}
    public static string ConfigureObservation(){return MorphForge.Editor.ParticleStage1Build.ConfigureObservation();}
    public static string RebuildOwned()
    {
        if(EditorApplication.isPlaying)throw new System.InvalidOperationException("Exit owned test first.");
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(scene.path!=MorphForge.Editor.ParticleStage1Build.ScenePath)throw new System.InvalidOperationException("Only the generated Stage1 scene may be rebuilt.");
        // Reload our generated scene to discard test-only framing/inspection changes.
        scene=UnityEditor.SceneManagement.EditorSceneManager.OpenScene(scene.path);
        UnityEditor.SceneManagement.EditorSceneManager.SaveScene(scene);
        return Build();
    }
    public static string ValidateExport()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Export check requires Edit Mode.");
        var source=AssetDatabase.LoadAssetAtPath<GameObject>(MorphForge.Editor.ParticleStage1Build.Root+"/Avatar.prefab");
        var clone=UnityEngine.Object.Instantiate(source);clone.hideFlags=HideFlags.HideAndDontSave;
        try
        {
            nadena.dev.ndmf.AvatarProcessor.ProcessAvatar(clone);
            var descriptor=clone.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>();
            var all=descriptor.expressionParameters.parameters;
            var ours=all.Where(p=>p.name.StartsWith("MF1_")).ToArray();
            var root=clone.transform.Find(MorphForge.Editor.ParticleStage1Build.ModuleName);
            var store=root.Find("Store").GetComponent<ParticleSystem>();
            var pump=root.Find("Minimum/Center/Pump").GetComponent<ParticleSystem>();
            int missing=clone.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));
            int bits=all.Where(p=>p.networkSynced).Sum(p=>p.valueType.ToString()=="Bool"?1:8);
            var ownController=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(MorphForge.Editor.ParticleStage1Build.Root+"/FX.controller");
            bool applyColorSpace=new SerializedObject(store.GetComponent<ParticleSystemRenderer>()).FindProperty("m_ApplyActiveColorSpace").boolValue;
            bool pass=ours.Length==11&&ours.All(p=>p.networkSynced&&p.valueType.ToString()=="Int")&&
                !all.Any(p=>p.name.StartsWith("MF1_W"))&&missing==0&&bits<=256&&
                pump.subEmitters.GetSubEmitterSystem(0)==store&&float.IsPositiveInfinity(store.main.startLifetime.constant)&&
                store.main.ringBufferMode==ParticleSystemRingBufferMode.Disabled&&!applyColorSpace&&
                store.GetComponent<ParticleSystemRenderer>().alignment==ParticleSystemRenderSpace.Local;
            var report=new{success=pass,scope="NDMF/MA Editor processing, not SDK upload or VRChat runtime",missingScripts=missing,
                sourceParameters=ours.Select(p=>new{p.name,type=p.valueType.ToString(),p.networkSynced}).ToArray(),
                workParametersInExpression=all.Count(p=>p.name.StartsWith("MF1_W")),addedSyncedBits=88,totalAvatarSyncedBits=bits,remainingBits=256-bits,
                controllerLayers=ownController.layers.Length,capacity=store.main.maxParticles,siblingSubEmitterRetained=pump.subEmitters.GetSubEmitterSystem(0)==store,
                infiniteLifetime=float.IsPositiveInfinity(store.main.startLifetime.constant),ringBuffer=store.main.ringBufferMode.ToString(),
                applyActiveColorSpace=applyColorSpace,renderAlignment=store.GetComponent<ParticleSystemRenderer>().alignment.ToString(),
                avatarCustomRuntimeScriptTypes=clone.GetComponentsInChildren<MonoBehaviour>(true).Where(b=>b&&b.GetType().Namespace!=null&&b.GetType().Namespace.StartsWith("MorphForge")).Select(b=>b.GetType().FullName).ToArray()};
            var json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);
            Directory.CreateDirectory("Docs/Stage1");File.WriteAllText("Docs/Stage1/export.json",json);
            if(!pass)throw new InvalidOperationException(json);return json;
        }
        finally{UnityEngine.Object.DestroyImmediate(clone);}
    }
}
