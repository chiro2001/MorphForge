using UnityEditor;
using System;
using System.IO;
using System.Linq;
using UnityEngine;

public static class ParticleStage2Entry
{
    public static string ActiveScene(){return UnityEngine.SceneManagement.SceneManager.GetActiveScene().path;}
    public static string StartOsc(){return MorphForge.Editor.ParticleStage2Harness.StartOsc();}
    public static string StopOsc(){return MorphForge.Editor.ParticleStage2Harness.StopOsc();}
    public static string Capture(string label,string expectedPath,bool images){return MorphForge.Editor.ParticleStage2Harness.Capture(label,expectedPath,images);}
    public static string CaptureFaces(string label,string expectedPath){return MorphForge.Editor.ParticleStage2Harness.CaptureFaces(label,expectedPath);}
    public static string FinishOwnedTest()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Exit the owned test first.");
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(scene.path!=MorphForge.Editor.ParticleStage2Build.ScenePath)throw new InvalidOperationException("Only the generated Stage2 test scene may be restored.");
        string path=scene.path;
        UnityEditor.SceneManagement.EditorSceneManager.CloseScene(scene,true);
        scene=UnityEditor.SceneManagement.EditorSceneManager.OpenScene(path,UnityEditor.SceneManagement.OpenSceneMode.Additive);
        UnityEngine.SceneManagement.SceneManager.SetActiveScene(scene);
        return Newtonsoft.Json.JsonConvert.SerializeObject(new{scene=scene.path,scene.isDirty,playMode=EditorApplication.isPlaying});
    }
    public static string Build(){return MorphForge.Editor.ParticleStage2Build.Build();}
    public static string ReadyForReview()
    {
        FinishOwnedTest();var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        var avatar=scene.GetRootGameObjects().First(g=>g.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>());
        Selection.activeGameObject=avatar;
        var sv=SceneView.lastActiveSceneView;
        if(sv){sv.drawGizmos=true;sv.sceneViewState.showParticleSystems=true;sv.LookAt(new Vector3(3,1.25f,0),Quaternion.LookRotation(new Vector3(-3,-2,-8)),4.5f,false,true);sv.Repaint();}
        var scenes=Enumerable.Range(0,UnityEngine.SceneManagement.SceneManager.sceneCount).Select(i=>UnityEngine.SceneManagement.SceneManager.GetSceneAt(i));
        var report=Newtonsoft.Json.JsonConvert.SerializeObject(new{ready=true,avatar=avatar.name,scene=scene.path,scene.isDirty,playMode=EditorApplication.isPlaying,
            hasExistingBlueprint=!string.IsNullOrEmpty(avatar.GetComponent<VRC.Core.PipelineManager>().blueprintId),
            openScenes=scenes.Select(s=>new{s.path,s.isDirty}).ToArray()},Newtonsoft.Json.Formatting.Indented);
        File.WriteAllText("Docs/Stage2/final-state.json",report);return report;
    }
    public static string ConfigureObservation(){return MorphForge.Editor.ParticleStage2Build.ConfigureObservation();}
    public static string DiagnoseAssets()
    {
        var root=MorphForge.Editor.ParticleStage2Build.Root;
        var fx=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(root+"/FX.controller");
        var assets=new[]{root+"/Display.prefab",root+"/Avatar.prefab"};
        return Newtonsoft.Json.JsonConvert.SerializeObject(new{fxLayers=fx.layers.Length,parameters=fx.parameters.Select(p=>p.name).ToArray(),
            assets=assets.Select(path=>new{path,merges=AssetDatabase.LoadAssetAtPath<GameObject>(path).GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true).Select(m=>new{m.name,controller=m.animator?AssetDatabase.GetAssetPath(m.animator):"NULL"}).ToArray()}).ToArray()},Newtonsoft.Json.Formatting.Indented);
    }
    public static string RepairControllerLinks()
    {
        var root=MorphForge.Editor.ParticleStage2Build.Root;
        var fx=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(root+"/FX.controller");
        foreach(var path in new[]{root+"/Display.prefab",root+"/Avatar.prefab"})
        {
            var prefab=PrefabUtility.LoadPrefabContents(path);
            try
            {
                foreach(var m in prefab.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true))
                {m.animator=fx;EditorUtility.SetDirty(m);PrefabUtility.RecordPrefabInstancePropertyModifications(m);}
                PrefabUtility.SaveAsPrefabAsset(prefab,path);
            }
            finally{PrefabUtility.UnloadPrefabContents(prefab);}
            AssetDatabase.ImportAsset(path,ImportAssetOptions.ForceUpdate);
            var saved=AssetDatabase.LoadAssetAtPath<GameObject>(path);
            foreach(var m in saved.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true))
            {
                var serialized=new SerializedObject(m);serialized.FindProperty("animator").objectReferenceValue=fx;serialized.ApplyModifiedPropertiesWithoutUndo();EditorUtility.SetDirty(m);
            }
            PrefabUtility.SavePrefabAsset(saved);
        }
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(scene.path==MorphForge.Editor.ParticleStage2Build.ScenePath)
        {
            foreach(var m in scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true)))
            {m.animator=fx;EditorUtility.SetDirty(m);PrefabUtility.RecordPrefabInstancePropertyModifications(m);}
            UnityEditor.SceneManagement.EditorSceneManager.MarkSceneDirty(scene);UnityEditor.SceneManagement.EditorSceneManager.SaveScene(scene);
        }
        AssetDatabase.SaveAssets();return DiagnoseAssets();
    }
    public static string RebuildOwned()
    {
        if(EditorApplication.isPlaying)throw new System.InvalidOperationException("Exit owned test first.");
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(scene.path!=MorphForge.Editor.ParticleStage2Build.ScenePath)throw new System.InvalidOperationException("Only the generated Stage2 scene may be rebuilt.");
        // Reload our generated scene to discard test-only framing/inspection changes.
        string path=scene.path;
        UnityEditor.SceneManagement.EditorSceneManager.CloseScene(scene,true);
        scene=UnityEditor.SceneManagement.EditorSceneManager.OpenScene(path,UnityEditor.SceneManagement.OpenSceneMode.Additive);
        UnityEngine.SceneManagement.SceneManager.SetActiveScene(scene);
        UnityEditor.SceneManagement.EditorSceneManager.SaveScene(scene);
        return Build();
    }
    public static string ValidateExport()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Export check requires Edit Mode.");
        var source=AssetDatabase.LoadAssetAtPath<GameObject>(MorphForge.Editor.ParticleStage2Build.Root+"/Avatar.prefab");
        var clone=UnityEngine.Object.Instantiate(source);clone.hideFlags=HideFlags.HideAndDontSave;
        try
        {
            nadena.dev.ndmf.AvatarProcessor.ProcessAvatar(clone);
            var descriptor=clone.GetComponent<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>();
            var all=descriptor.expressionParameters.parameters;
            var ours=all.Where(p=>p.name.StartsWith("MF2_")).ToArray();
            var root=clone.transform.Find(MorphForge.Editor.ParticleStage2Build.ModuleName);
            var store=root.Find("Store").GetComponent<ParticleSystem>();
            var pump=root.Find("Minimum/Center/Pump").GetComponent<ParticleSystem>();
            int missing=clone.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));
            int bits=all.Where(p=>p.networkSynced).Sum(p=>p.valueType.ToString()=="Bool"?1:8);
            var ownController=AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(MorphForge.Editor.ParticleStage2Build.Root+"/FX.controller");
            bool applyColorSpace=new SerializedObject(store.GetComponent<ParticleSystemRenderer>()).FindProperty("m_ApplyActiveColorSpace").boolValue;
            var renderer=store.GetComponent<ParticleSystemRenderer>();
            var streams=new System.Collections.Generic.List<ParticleSystemVertexStream>();renderer.GetActiveVertexStreams(streams);
            bool pass=ours.Length==11&&ours.All(p=>p.networkSynced&&p.valueType.ToString()=="Int")&&
                !all.Any(p=>p.name.StartsWith("MF2_W"))&&missing==0&&bits<=256&&
                pump.subEmitters.GetSubEmitterSystem(0)==store&&float.IsPositiveInfinity(store.main.startLifetime.constant)&&
                store.main.ringBufferMode==ParticleSystemRingBufferMode.Disabled&&!applyColorSpace&&
                renderer.alignment==ParticleSystemRenderSpace.Local&&ownController.layers.Length==3&&
                renderer.sharedMaterial.shader.name=="MorphForge/ParticleBoxLocalUV"&&streams.Contains(ParticleSystemVertexStream.SizeXYZ)&&
                streams.Contains(ParticleSystemVertexStream.UV2)&&renderer.mesh.uv2.Length==renderer.mesh.vertexCount;
            var report=new{success=pass,scope="NDMF/MA Editor processing, not SDK upload or VRChat runtime",missingScripts=missing,
                sourceParameters=ours.Select(p=>new{p.name,type=p.valueType.ToString(),p.networkSynced}).ToArray(),
                workParametersInExpression=all.Count(p=>p.name.StartsWith("MF2_W")),addedSyncedBits=88,totalAvatarSyncedBits=bits,remainingBits=256-bits,
                controllerLayers=ownController.layers.Length,capacity=store.main.maxParticles,siblingSubEmitterRetained=pump.subEmitters.GetSubEmitterSystem(0)==store,
                infiniteLifetime=float.IsPositiveInfinity(store.main.startLifetime.constant),ringBuffer=store.main.ringBufferMode.ToString(),
                applyActiveColorSpace=applyColorSpace,renderAlignment=renderer.alignment.ToString(),
                shader=renderer.sharedMaterial.shader.name,vertexStreams=streams.Select(s=>s.ToString()).ToArray(),
                protocol="MF2/single-commit",sequenceBits=5,inboxCapacity=1,hasExistingBlueprint=!string.IsNullOrEmpty(clone.GetComponent<VRC.Core.PipelineManager>().blueprintId),
                avatarCustomRuntimeScriptTypes=clone.GetComponentsInChildren<MonoBehaviour>(true).Where(b=>b&&b.GetType().Namespace!=null&&b.GetType().Namespace.StartsWith("MorphForge")).Select(b=>b.GetType().FullName).ToArray()};
            var json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);
            Directory.CreateDirectory("Docs/Stage2");File.WriteAllText("Docs/Stage2/export.json",json);
            if(!pass)throw new InvalidOperationException(json);return json;
        }
        finally{UnityEngine.Object.DestroyImmediate(clone);}
    }
}
