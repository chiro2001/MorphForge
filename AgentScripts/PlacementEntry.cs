using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Collections.Generic;
using UnityEditor;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3.Dynamics.Constraint.Components;
using VRC.SDK3.Dynamics.Contact.Components;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class PlacementEntry
{
    const BindingFlags Flags=BindingFlags.Public|BindingFlags.NonPublic|BindingFlags.Instance;
    static float[] V(Vector3 v){return new[]{v.x,v.y,v.z};}
    static GestureManager Manager(){return UnityEngine.Object.FindObjectsOfType<GestureManager>().First(g=>g.name=="MorphForge Stage3 Gesture Manager");}
    static ModuleVrc3 Module()
    {
        var g=Manager();if(g.Module==null)g.SetModule(new ModuleVrc3(g.settings.favourite.GetComponent<VRCAvatarDescriptor>()));
        return g.Module as ModuleVrc3;
    }
    static Transform Named(Transform root,string name){return root.GetComponentsInChildren<Transform>(true).First(t=>t.name==name);}
    public static string Build(){return MorphForge.Editor.PlacementBuild.Build();}
    public static string RebuildGenerated()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Requires Edit Mode");
        for(int i=UnityEngine.SceneManagement.SceneManager.sceneCount-1;i>=0;i--)
        {
            var scene=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);
            if(scene.path==MorphForge.Editor.PlacementBuild.ScenePath)UnityEditor.SceneManagement.EditorSceneManager.CloseScene(scene,true);
        }
        return Build();
    }
    public static string StartOsc()
    {
        if(!EditorApplication.isPlaying)throw new InvalidOperationException("Enter Play Mode first");
        for(int i=0;i<UnityEngine.SceneManagement.SceneManager.sceneCount;i++)
        {
            var s=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);if(s.path==MorphForge.Editor.PlacementBuild.ScenePath)continue;
            foreach(var root in s.GetRootGameObjects())root.SetActive(false);
        }
        var m=Module();var osc=typeof(ModuleVrc3).GetField("OscModule",Flags).GetValue(m);
        if(!(bool)osc.GetType().GetProperty("Enabled",Flags).GetValue(osc))osc.GetType().GetMethod("Start",Flags).Invoke(osc,new object[]{9012,"127.0.0.1",9013});
        return Capture();
    }
    public static string StopOsc()
    {
        var m=Module();var osc=typeof(ModuleVrc3).GetField("OscModule",Flags).GetValue(m);osc.GetType().GetMethod("Stop",Flags).Invoke(osc,new object[]{true});return "OSC stopped";
    }
    public static string ReadyForReview()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Exit Play Mode first");
        for(int i=UnityEngine.SceneManagement.SceneManager.sceneCount-1;i>=0;i--)
        {
            var scene=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);
            if(scene.path==MorphForge.Editor.PlacementBuild.ScenePath)UnityEditor.SceneManagement.EditorSceneManager.CloseScene(scene,true);
        }
        var target=UnityEditor.SceneManagement.EditorSceneManager.OpenScene(MorphForge.Editor.PlacementBuild.ScenePath,UnityEditor.SceneManagement.OpenSceneMode.Additive);
        UnityEngine.SceneManagement.SceneManager.SetActiveScene(target);
        var avatar=target.GetRootGameObjects().First(g=>g.GetComponent<VRCAvatarDescriptor>());Selection.activeGameObject=avatar;
        // Editor-only visibility prevents coincident observation floors and Avatars
        // in the retained Stage1/2 scenes from obscuring this module.
        for(int i=0;i<UnityEngine.SceneManagement.SceneManager.sceneCount;i++)
        {
            var scene=UnityEngine.SceneManagement.SceneManager.GetSceneAt(i);
            if(scene.path==MorphForge.Editor.PlacementBuild.ScenePath||!scene.path.StartsWith("Assets/MorphForge/Scenes/"))continue;
            foreach(var root in scene.GetRootGameObjects())SceneVisibilityManager.instance.Hide(root,true);
        }
        var sv=SceneView.lastActiveSceneView;if(sv){sv.drawGizmos=true;sv.sceneViewState.showParticleSystems=true;sv.LookAt(new Vector3(0,.6f,.15f),Quaternion.LookRotation(new Vector3(-2,-1,-4)),1.1f,false,true);sv.Repaint();}
        var scenes=Enumerable.Range(0,UnityEngine.SceneManagement.SceneManager.sceneCount).Select(i=>UnityEngine.SceneManagement.SceneManager.GetSceneAt(i));
        var json=Newtonsoft.Json.JsonConvert.SerializeObject(new{ready=true,avatar=avatar.name,scene=target.path,target.isDirty,playMode=EditorApplication.isPlaying,
            hasExistingBlueprint=!string.IsNullOrEmpty(avatar.GetComponent<VRC.Core.PipelineManager>().blueprintId),openScenes=scenes.Select(s=>new{s.path,s.isDirty}).ToArray()},Newtonsoft.Json.Formatting.Indented);
        Directory.CreateDirectory("Docs/Stage3");File.WriteAllText("Docs/Stage3/final-state.json",json);return json;
    }
    public static string Screenshot()
    {
        var camera=Camera.main;if(!camera)throw new InvalidOperationException("No active camera");
        var old=camera.targetTexture;var active=RenderTexture.active;var rt=RenderTexture.GetTemporary(1280,720,24);var tex=new Texture2D(1280,720,TextureFormat.RGB24,false);
        try{camera.targetTexture=rt;camera.Render();RenderTexture.active=rt;tex.ReadPixels(new Rect(0,0,1280,720),0,0);tex.Apply();Directory.CreateDirectory("Docs/Stage3");File.WriteAllBytes("Docs/Stage3/observation-game.png",tex.EncodeToPNG());return "Docs/Stage3/observation-game.png";}
        finally{camera.targetTexture=old;RenderTexture.active=active;RenderTexture.ReleaseTemporary(rt);UnityEngine.Object.DestroyImmediate(tex);}
    }
    public static string Capture()
    {
        var m=Module();var root=m.Avatar.transform;var anchor=Named(root,"AnchorPose");var scale=anchor.Find("ContentScale");var store=Named(root,"Store").GetComponent<ParticleSystem>();
        var parts=new ParticleSystem.Particle[store.main.maxParticles];int count=store.GetParticles(parts);var constraint=anchor.GetComponent<VRCParentConstraint>();
        var render=store.GetComponent<ParticleSystemRenderer>();
        var mesh=new Mesh();var camera=Camera.main;render.BakeMesh(mesh,camera,true);var meshBounds=mesh.bounds;UnityEngine.Object.DestroyImmediate(mesh);
        return Newtonsoft.Json.JsonConvert.SerializeObject(new{avatar=V(root.position),anchor=V(anchor.position),anchorEuler=V(anchor.eulerAngles),anchorScale=V(anchor.lossyScale),contentScale=V(scale.localScale),contentWorldScale=V(scale.lossyScale),
            frozen=constraint.FreezeToWorld,rebake=constraint.RebakeOffsetsWhenUnfrozen,world=m.GetParam("MF_World").BoolValue(),handle=m.GetParam("MF_Handle").BoolValue(),grab=m.GetParam("MF_Grab").IntValue(),
            scaleParameter=m.GetParam("MF_Scale").FloatValue(),isLocal=m.GetParam("IsLocal").BoolValue(),
            leftHand=V(Named(root,"LeftHand").position),rightHand=V(Named(root,"RightHand").position),count,
            received=m.GetParam("MF2_RxCount").FloatValue(),completed=m.GetParam("MF2_DoneCount").FloatValue(),
            boundsCenter=V(render.bounds.center),boundsSize=V(render.bounds.size),
            renderedMeshSize=V(meshBounds.size),
            particles=parts.Take(count).Select(p=>new{position=V(p.position),size=V(p.startSize3D),worldPosition=V(store.main.customSimulationSpace.TransformPoint(p.position)),codes=new[]{Mathf.RoundToInt(p.startColor.r*63f/255),Mathf.RoundToInt(p.startColor.g*63f/255),Mathf.RoundToInt(p.startColor.b*63f/255),Mathf.RoundToInt(p.startColor.a*63f/255)}}).ToArray()},Newtonsoft.Json.Formatting.Indented);
    }
    public static string ValidateExport()
    {
        if(EditorApplication.isPlaying)throw new InvalidOperationException("Requires Edit Mode");
        var clone=UnityEngine.Object.Instantiate(AssetDatabase.LoadAssetAtPath<GameObject>(MorphForge.Editor.PlacementBuild.Root+"/Avatar.prefab"));clone.hideFlags=HideFlags.HideAndDontSave;
        try
        {
            nadena.dev.ndmf.AvatarProcessor.ProcessAvatar(clone);var desc=clone.GetComponent<VRCAvatarDescriptor>();var parameters=desc.expressionParameters.parameters;
            var anchor=Named(clone.transform,"AnchorPose");var store=Named(clone.transform,"Store").GetComponent<ParticleSystem>();
            int missing=clone.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));
            int bits=parameters.Where(p=>p.networkSynced).Sum(p=>p.valueType.ToString()=="Bool"?1:8);
            var expected=new[]{"MF_World","MF_Handle","MF_Scale","MF_Reset","MF_Grab"};
            var expression=parameters.Where(p=>expected.Contains(p.name)).Select(p=>new{p.name,type=p.valueType.ToString(),p.networkSynced,p.defaultValue}).ToArray();
            var contacts=clone.GetComponentsInChildren<VRCContactReceiver>(true).Where(c=>c.parameter.StartsWith("MF_Near")).ToArray();
            var menuNames=new List<string>();var seen=new HashSet<VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu>();
            void Walk(VRC.SDK3.Avatars.ScriptableObjects.VRCExpressionsMenu menu)
            {if(!menu||!seen.Add(menu))return;foreach(var control in menu.controls){menuNames.Add(control.name);Walk(control.subMenu);}}
            Walk(desc.expressionsMenu);
            var scripts=clone.GetComponentsInChildren<MonoBehaviour>(true).Where(b=>b&&b.GetType().Namespace!=null&&b.GetType().Namespace.StartsWith("MorphForge")).Select(b=>b.GetType().FullName).ToArray();
            var report=new{success=missing==0&&bits<=256&&expression.Length==5&&contacts.Length==2&&scripts.Length==0&&menuNames.Contains("MorphForge")&&anchor.GetComponent<VRCParentConstraint>()&&store.main.customSimulationSpace,
                missingScripts=missing,syncedBits=bits,remainingBits=256-bits,placementParameters=expression,menuNames,contacts=contacts.Select(c=>new{c.parameter,c.localOnly,c.allowSelf,c.allowOthers,c.radius,tags=c.collisionTags}).ToArray(),
                customAvatarScripts=scripts,particleCapacity=store.main.maxParticles,scalingMode=store.main.scalingMode.ToString(),
                constraints=clone.GetComponentsInChildren<VRCParentConstraint>(true).Select(c=>new{c.name,c.FreezeToWorld,c.RebakeOffsetsWhenUnfrozen}).ToArray(),
                scope="NDMF Editor processing, not a VRChat client pickup test"};
            string json=Newtonsoft.Json.JsonConvert.SerializeObject(report,Newtonsoft.Json.Formatting.Indented);Directory.CreateDirectory("Docs/Stage3");File.WriteAllText("Docs/Stage3/export.json",json);return json;
        }
        finally{UnityEngine.Object.DestroyImmediate(clone);}
    }
}
