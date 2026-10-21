using System;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;
using VRC.SDK3.Dynamics.Contact.Components;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using MorphForge.Editor;
using MorphForgeAvatar.Editor;
using Newtonsoft.Json.Linq;
public static class TextAnimationEntry {
    public static async Task<string> CheckPlacement() {
        if(!EditorApplication.isPlaying)throw new Exception("Play Mode required");
        var gm=UnityEngine.Object.FindObjectsOfType<GestureManager>().Single(g=>g.gameObject.scene.path==Mf6AvatarBuild.ScenePath);
        var m=gm.Module as ModuleVrc3;if(m==null)throw new Exception("GestureManager not ready");
        var anchor=m.Avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="AnchorPose");
        var visual=anchor.Find("HandleVisual");var sensors=anchor.Find("Sensors");
        var receivers=anchor.GetComponentsInChildren<VRCContactReceiver>(true);var enabled=receivers.Select(r=>r.enabled).ToArray();
        var names=new[]{"MF_Handle","MF_AllowGrab","GestureLeft","GestureRight","MF_NearL","MF_NearR","MF_Reset","MF_Grab"};
        var old=names.Select(n=>m.GetParam(n).FloatValue()).ToArray();var checks=new JArray();
        void Set(string n,float v){m.GetParam(n).Set(m,v);}
        async Task Tick(){await Task.Delay(450);}
        void Check(string name,bool pass){checks.Add(new JObject{["name"]=name,["pass"]=pass});}
        try {
            foreach(var r in receivers)r.enabled=false;
            Set("MF_Reset",0);Set("GestureRight",0);Set("GestureLeft",0);Set("MF_NearR",0);Set("MF_NearL",0);Set("MF_Grab",0);await Tick();
            foreach(bool visible in new[]{true,false})foreach(bool grab in new[]{true,false}) {
                Set("MF_Handle",visible?1:0);Set("MF_AllowGrab",grab?1:0);Set("GestureLeft",0);Set("MF_NearL",0);await Tick();
                Set("MF_NearL",1);Set("GestureLeft",1);await Tick();
                Check("visible="+visible+" allowed="+grab,visual.gameObject.activeInHierarchy==visible&&sensors.gameObject.activeInHierarchy&&m.GetParam("MF_Grab").IntValue()==(grab?1:0));
                Set("GestureLeft",0);Set("MF_NearL",0);await Tick();
            }
            Set("MF_Handle",1);Set("MF_AllowGrab",1);Set("MF_NearL",1);Set("GestureLeft",1);await Tick();Set("MF_Handle",0);await Tick();
            Check("hiding sphere preserves held hand",m.GetParam("MF_Grab").IntValue()==1&&!visual.gameObject.activeInHierarchy);
            var before=anchor.position;Set("MF_AllowGrab",0);await Tick();
            Check("disabling grab releases at current position",m.GetParam("MF_Grab").IntValue()==0&&Vector3.Distance(before,anchor.position)<.02f);
            var dock=m.Avatar.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="AvatarDock");
            Check("front dock faces avatar without negative scale",Vector3.Dot(dock.forward,m.Avatar.transform.forward)<-.99f&&dock.localScale.x>0);
            var result=new JObject{["success"]=checks.All(c=>(bool)c["pass"]),["checks"]=checks,["scope"]="Actual merged GestureManager Animator with controlled self-contact parameters; no physical VR or remote-player claim."};
            Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/placement-play.json",result.ToString());return result.ToString();
        }finally{for(int i=0;i<names.Length;i++)Set(names[i],old[i]);for(int i=0;i<receivers.Length;i++)receivers[i].enabled=enabled[i];}
    }
    public static string RunText() {
        foreach(var d in EditorApplication.delayCall?.GetInvocationList()??Array.Empty<Delegate>())if(d.Method.DeclaringType?.FullName?.StartsWith("TextAnimationEntry")==true)EditorApplication.delayCall-=(EditorApplication.CallbackFunction)d;
        foreach(var d in EditorApplication.update?.GetInvocationList()??Array.Empty<Delegate>())if(d.Method.DeclaringType?.FullName?.StartsWith("TextAnimationEntry")==true)EditorApplication.update-=(EditorApplication.CallbackFunction)d;
        try{var result=Text();File.WriteAllText("Docs/TextAnimation/text-build.json",result);return result;}
        catch(Exception e){File.WriteAllText("Docs/TextAnimation/text-build.json",new JObject{["success"]=false,["error"]=e.ToString()}.ToString());throw;}
    }
    public static string Text() {
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(EditorApplication.isPlaying||scene.isDirty||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Clean saved MF6 scene in Edit Mode required");
        Mf6TextBuild.BuildAssets();
        foreach(var path in new[]{Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath}) {
            var root=PrefabUtility.LoadPrefabContents(path);
            try {var module=path==AvatarSetupWindow.PrefabPath?root:root.transform.Find(PlacementStage4Build.ModuleName).gameObject;Mf6TextBuild.Install(module);
                var catalogPath=path==AvatarSetupWindow.PrefabPath?AvatarSetupWindow.Root+"/CatalogSource/catalog.json":"Docs/MF6/avatar-catalog.json";
                var catalog=JObject.Parse(File.ReadAllText(catalogPath));var caps=(JArray)catalog["capabilities"];foreach(var capability in new[]{"mf6-text-panels/1","mf6-text-latest14/1","local-placement-controls/1"})if(!caps.Any(c=>(string)c==capability))caps.Add(capability);
                if(path==AvatarSetupWindow.PrefabPath){var identity=CatalogIdentity.Apply(module,catalog);root.GetComponent<MorphForgeAvatar.MorphForgeAvatarModule>().catalogIdentity=identity;}else AvatarCapabilitiesBuild.Apply(root,catalog);
                File.WriteAllText(catalogPath,catalog.ToString());PrefabUtility.SaveAsPrefabAsset(root,path);}
            finally{PrefabUtility.UnloadPrefabContents(root);}
        }
        var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>(true)).Single();
        var center=avatar.GetComponentsInChildren<nadena.dev.modular_avatar.core.ModularAvatarMergeAnimator>(true).Single(m=>m.gameObject.name=="MF6Decoder").transform.parent;
        foreach(var child in center.Cast<Transform>().Where(t=>t.name=="TextDisplay").ToArray())
            if(PrefabUtility.IsAddedGameObjectOverride(child.gameObject))PrefabUtility.RevertAddedGameObject(child.gameObject,InteractionMode.AutomatedAction);
        if(center.Cast<Transform>().Count(t=>t.name=="TextDisplay")!=1)throw new Exception("Scene must inherit exactly one TextDisplay; duplicate overrides were removed, source prefab is preserved");
        EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();
        return new JObject{["success"]=true,["panels"]=Mf6TextBuild.Panels,["slotsPerPanel"]=128,["additionalSyncBits"]=0}.ToString();
    }
    public static string Placement() {
        var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        if(EditorApplication.isPlaying||scene.isDirty||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Clean saved MF6 scene in Edit Mode required");
        foreach(var path in new[]{Mf6AvatarBuild.AvatarPath,AvatarSetupWindow.PrefabPath}) {
            var root=PrefabUtility.LoadPrefabContents(path);
            try {var module=path==AvatarSetupWindow.PrefabPath?root:root.transform.Find(PlacementStage4Build.ModuleName).gameObject;PlacementControls.Apply(module);PrefabUtility.SaveAsPrefabAsset(root,path);}
            finally{PrefabUtility.UnloadPrefabContents(root);}
        }
        var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>(true)).Single();
        PlacementControls.Apply(avatar.transform.Find(PlacementStage4Build.ModuleName).gameObject);
        EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();
        var result=new JObject { ["success"]=true,["allowGrabParameter"]=PlacementControls.AllowGrab,["additionalSyncBits"]=0,["dockYaw"]=180,["visibilityDisablesContacts"]=false };
        Directory.CreateDirectory("Docs/TextAnimation");File.WriteAllText("Docs/TextAnimation/placement.json",result.ToString());return result.ToString();
    }
}
