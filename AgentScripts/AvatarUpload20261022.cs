using System;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using UnityEngine.UIElements;
using MorphForge.Editor;
using nadena.dev.modular_avatar.core;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3A.Editor;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;

// Explicitly authorized update of the current saved MF6 Avatar, not a new ID.
public static class AvatarUpload20261022 {
    const string Folder="Docs/AvatarUpload/20261022";
    static string StatePath=>Folder+"/upload-state.json";
    static JObject State()=>File.Exists(StatePath)?JObject.Parse(File.ReadAllText(StatePath)):new JObject();
    static void Save(JObject value){Directory.CreateDirectory(Folder);File.WriteAllText(StatePath,value.ToString());}
    static void Progress(string phase,string message,float? fraction=null){var s=State();s["phase"]=phase;s["message"]=message;s["progress"]=fraction;s["updatedUtc"]=DateTime.UtcNow.ToString("O");Save(s);}
    static GameObject Target(){
        if(Application.dataPath.Replace('\\','/')!="D:/Projects/VRChatProjects2/MorphForge/Assets")throw new Exception("Wrong Unity project");
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        var scene=SceneManager.GetActiveScene();
        if(scene.path!=Mf6AvatarBuild.ScenePath||scene.isDirty)throw new Exception("The saved MF6 source scene must be active and clean");
        for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("Unsaved scene preserved");
        var avatars=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).ToArray();
        if(avatars.Length!=1)throw new Exception("Exactly one source Avatar required");
        var avatar=avatars[0].gameObject;var pipeline=avatar.GetComponent<PipelineManager>();
        if(avatar.name!="MorphForge_MF6_Avatar"||!pipeline||string.IsNullOrEmpty(pipeline.blueprintId)||pipeline.blueprintId!=Mf6AvatarBuild.AvatarId)throw new Exception("Existing scene/prefab Avatar identity mismatch");
        if(EditorUserBuildSettings.activeBuildTarget!=BuildTarget.StandaloneWindows64)throw new Exception("Current PC Avatar requires StandaloneWindows64");
        return avatar;
    }
    public static string Inspect(){
        var avatar=Target();var merges=avatar.GetComponentsInChildren<ModularAvatarMergeAnimator>(true);
        var decoder=merges.Single(m=>m.name=="MF6Decoder").animator as AnimatorController;
        var renderers=avatar.GetComponentsInChildren<Renderer>(true);
        var stores=avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").ToArray();
        var text=renderers.Where(r=>r.sharedMaterials.Any(m=>m&&m.shader&&m.shader.name.Contains("PixelText16"))).ToArray();
        var motions=merges.Where(m=>m.animator&&AssetDatabase.GetAssetPath(m.animator).EndsWith("/Motion.controller")).ToArray();
        var preview=avatar.GetComponentsInChildren<Camera>(true).Where(c=>c.targetTexture).ToArray();
        var missing=avatar.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));
        var flags=decoder.parameters.Single(p=>p.name=="MF_InfoFeatures").defaultInt;
        var shaders=renderers.SelectMany(r=>r.sharedMaterials).Where(m=>m&&m.shader&&m.shader.name.StartsWith("MorphForge")).Select(m=>m.shader).Distinct().ToArray();
        var result=new JObject {
            ["success"]=missing==0&&stores.Length==8&&stores.All(p=>p.main.maxParticles==2056)&&text.Length==4&&motions.Length==1&&preview.Length>=1&&(flags&119)==119&&(flags&~127)==0&&shaders.All(s=>!ShaderUtil.ShaderHasError(s)),
            ["scene"]=avatar.scene.path,["sceneDirty"]=avatar.scene.isDirty,["avatarName"]=avatar.name,["avatarId"]=avatar.GetComponent<PipelineManager>().blueprintId,
            ["prefabDependencyHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["sceneDependencyHash"]=AssetDatabase.GetAssetDependencyHash(avatar.scene.path).ToString(),
            ["stores"]=stores.Length,["physicalParticles"]=stores.Sum(p=>p.main.maxParticles),["missingScripts"]=missing,["receiverFlags"]=flags,
            ["textRenderers"]=text.Length,["motionControllers"]=motions.Length,["previewCameras"]=preview.Length,
            ["previewTargets"]=new JArray(preview.Select(c=>(JToken)new JObject{["name"]=c.name,["width"]=c.targetTexture.width,["height"]=c.targetTexture.height})),
            ["shaderErrors"]=new JArray(shaders.Where(ShaderUtil.ShaderHasError).Select(s=>s.name)),
            ["controllers"]=new JArray(merges.Select(m=>(JToken)new JObject{["object"]=m.name,["path"]=AssetDatabase.GetAssetPath(m.animator)})),
            ["sdkAvatarVersion"]=UnityEditor.PackageManager.PackageInfo.GetAllRegisteredPackages().FirstOrDefault(p=>p.name=="com.vrchat.avatars")?.version,
            ["timeUtc"]=DateTime.UtcNow.ToString("O")
        };
        Directory.CreateDirectory(Folder);File.WriteAllText(Folder+"/source-inspection.json",result.ToString());return result.ToString();
    }
    static async Task<VRCAvatar> RemoteAvatar(GameObject avatar){
        if(APIUser.CurrentUser==null&&ApiCredentials.Load())APIUser.InitialFetchCurrentUser(_=>{},null);
        for(int i=0;i<60&&APIUser.CurrentUser==null;i++)await Task.Delay(250);
        var user=APIUser.CurrentUser;
        if(user==null||!user.canPublishAvatars)throw new Exception("SDK publisher login is required");
        var remote=await VRCApi.GetAvatar(avatar.GetComponent<PipelineManager>().blueprintId,forceRefresh:true);
        if(remote.AuthorId!=user.id||remote.ReleaseStatus!="private"||remote.PendingUpload)throw new Exception("Expected the existing fully uploaded private Avatar owned by the current SDK user");
        return remote;
    }
    public static async Task<string> Remote(){
        var remote=await RemoteAvatar(Target());var result=new JObject{["avatarId"]=remote.ID,["name"]=remote.Name,["version"]=remote.Version,["releaseStatus"]=remote.ReleaseStatus,["pendingUpload"]=remote.PendingUpload,["ownedBySdkUser"]=true,["timeUtc"]=DateTime.UtcNow.ToString("O")};
        Directory.CreateDirectory(Folder);File.WriteAllText(Folder+"/remote-before.json",result.ToString());return result.ToString();
    }
    public static async Task<string> Start(){
        var avatar=Target();var inspection=JObject.Parse(Inspect());if((bool?)inspection["success"]!=true)throw new Exception("Current source does not contain all validated features");
        if((string)State()["status"]=="running")throw new Exception("This upload is already running; inspect status instead");
        var remote=await RemoteAvatar(avatar);
        if(!VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi builder)){EditorWindow.GetWindow<VRCSdkControlPanel>(false,"VRChat SDK",true).Show();if(!VRCSdkControlPanel.TryGetBuilder(out builder))throw new Exception("SDK Avatar builder unavailable");}
        if(builder.BuildState!=SdkBuildState.Idle||builder.UploadState!=SdkUploadState.Idle)throw new Exception("SDK is busy; preserve the current operation");
        builder.SelectAvatar(avatar);
        var operation=Guid.NewGuid().ToString("N");
        Save(new JObject{["status"]="running",["phase"]="queued",["operation"]=operation,["avatarId"]=remote.ID,["avatarName"]=remote.Name,["releaseStatus"]=remote.ReleaseStatus,["versionBefore"]=remote.Version,["startedUtc"]=DateTime.UtcNow.ToString("O"),["sourceInspection"]=inspection});
        EditorApplication.CallbackFunction next=null;next=()=>{EditorApplication.update-=next;Upload(builder,avatar,remote,operation);};EditorApplication.update+=next;EditorApplication.QueuePlayerLoopUpdate();return State().ToString();
    }
    static async void Upload(IVRCSdkAvatarBuilderApi builder,GameObject avatar,VRCAvatar remote,string operation){
        EventHandler<string> build=(sender,message)=>Progress("build",message);
        EventHandler<(string status,float percentage)> upload=(sender,value)=>Progress("upload",value.status,value.percentage);
        builder.OnSdkBuildProgress+=build;builder.OnSdkUploadProgress+=upload;
        try {
            Progress("sdk-invoked","Public SDK BuildAndUpload on the current existing Avatar");
            await builder.BuildAndUpload(avatar,remote,null,CancellationToken.None);
            var latest=await VRCApi.GetAvatar(remote.ID,forceRefresh:true);
            if(latest.ID!=remote.ID||latest.ReleaseStatus!=remote.ReleaseStatus||latest.Name!=remote.Name||latest.PendingUpload)throw new Exception("Remote confirmation differs from the requested existing Avatar");
            var s=State();if((string)s["operation"]!=operation)throw new Exception("Upload operation state changed");
            s["status"]="success";s["phase"]="complete";s["versionAfter"]=latest.Version;s["pendingUpload"]=latest.PendingUpload;s["sceneDirtyAfter"]=avatar.scene.isDirty;s["sceneAvatarIdAfter"]=avatar.GetComponent<PipelineManager>().blueprintId;s["completedUtc"]=DateTime.UtcNow.ToString("O");Save(s);
        }catch(Exception e){var s=State();s["status"]="failed";s["error"]=e.ToString();s["completedUtc"]=DateTime.UtcNow.ToString("O");Save(s);Debug.LogError("MorphForge current Avatar upload failed: "+e.Message);}
        finally{builder.OnSdkBuildProgress-=build;builder.OnSdkUploadProgress-=upload;}
    }
    public static string Status()=>State().ToString();
    public static string SdkStatus(){
        bool found=VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi builder);
        var result=new JObject{["available"]=found,["buildState"]=found?builder.BuildState.ToString():null,["uploadState"]=found?builder.UploadState.ToString():null,["selectedAvatar"]=found&&builder.SelectedAvatar?builder.SelectedAvatar.name:null,["timeUtc"]=DateTime.UtcNow.ToString("O")};
        return result.ToString();
    }
    public static string SdkUi(){
        var root=VRCSdkControlPanel.window?.rootVisualElement;
        if(root==null)throw new Exception("SDK window unavailable");
        return new JObject{["texts"]=new JArray(root.Query<Label>().ToList().Where(l=>l.visible).Select(l=>l.text).Where(t=>!string.IsNullOrWhiteSpace(t)).TakeLast(35)),["buttons"]=new JArray(root.Query<Button>().ToList().Where(b=>b.visible&&b.enabledInHierarchy).Select(b=>(JToken)new JObject{["text"]=b.text,["name"]=b.name,["parent"]=b.parent?.GetType().FullName,["parentName"]=b.parent?.name}))}.ToString();
    }
    public static string ContinueAuthorizedUpload(){
        var avatar=Target();var state=State();
        if((string)state["status"]!="running"||(string)state["avatarId"]!=avatar.GetComponent<PipelineManager>().blueprintId)throw new Exception("No matching authorized upload is waiting");
        var root=VRCSdkControlPanel.window?.rootVisualElement;if(root==null)throw new Exception("SDK window unavailable");
        var modal=root.Query<VRC.SDKBase.Editor.Elements.Modal>().ToList().Single(m=>m.IsOpen&&m.Q<Label>("modal-title")?.text=="Copyright ownership agreement");
        if(!modal.Query<Label>().ToList().Any(l=>l.text==VRC.SDKBase.VRCCopyrightAgreement.AgreementText))throw new Exception("Unexpected copyright confirmation text");
        var button=modal.Q<Button>("modal-action-button");if(button==null||button.text!="OK"||!button.enabledInHierarchy)throw new Exception("SDK agreement confirmation is unavailable");
        // Dispatch the SDK's normal confirmation UI for this explicitly requested
        // update of the user's existing private Avatar. Never alter consent caches.
        var click=typeof(Clickable).GetMethod("SimulateSingleClick",System.Reflection.BindingFlags.Instance|System.Reflection.BindingFlags.NonPublic,null,new[]{typeof(EventBase),typeof(int)},null);
        if(click==null)throw new Exception("Unity 2022 button click simulator is unavailable");
        click.Invoke(button.clickable,new object[]{null,0});
        return new JObject{["confirmationDispatched"]=true,["avatarId"]=state["avatarId"],["sdkModalStillOpen"]=modal.IsOpen}.ToString();
    }
    static GameObject CompletedTarget(){
        var s=State();if((string)s["status"]!="success")throw new Exception("Upload has not succeeded");
        var scene=SceneManager.GetActiveScene();if(EditorApplication.isPlaying||scene.path!=Mf6AvatarBuild.ScenePath)throw new Exception("Source MF6 scene required");
        var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
        if(avatar.GetComponent<PipelineManager>().blueprintId!=(string)s["avatarId"])throw new Exception("Post-upload identity changed");return avatar;
    }
    public static string Postflight(){
        var avatar=CompletedTarget();var mods=PrefabUtility.GetPropertyModifications(avatar)??new PropertyModification[0];
        var result=new JObject{["scene"]=avatar.scene.path,["sceneDirty"]=avatar.scene.isDirty,["avatarId"]=avatar.GetComponent<PipelineManager>().blueprintId,["prefabId"]=Mf6AvatarBuild.AvatarId,
            ["prefabDependencyHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["sceneDependencyHash"]=AssetDatabase.GetAssetDependencyHash(avatar.scene.path).ToString(),
            ["rootNames"]=new JArray(avatar.scene.GetRootGameObjects().Select(g=>g.name)),["overrideCount"]=mods.Length,
            ["pipelineOverrides"]=new JArray(mods.Where(m=>m.target is PipelineManager).Select(m=>(JToken)new JObject{["path"]=m.propertyPath,["value"]=m.value})),
            ["dirtyAvatarComponents"]=new JArray(avatar.GetComponentsInChildren<Component>(true).Where(c=>c&&EditorUtility.IsDirty(c)).Select(c=>c.GetType().FullName+":"+c.name).Take(20))};
        File.WriteAllText(Folder+"/postflight.json",result.ToString());return result.ToString();
    }
    public static async Task<string> RemoteAfter(){
        var remote=await RemoteAvatar(CompletedTarget());var result=new JObject{["avatarId"]=remote.ID,["name"]=remote.Name,["version"]=remote.Version,["releaseStatus"]=remote.ReleaseStatus,["pendingUpload"]=remote.PendingUpload,["ownedBySdkUser"]=true,["timeUtc"]=DateTime.UtcNow.ToString("O")};
        if(remote.Version<=(int)State()["versionBefore"])throw new Exception("Remote Avatar revision did not increase");
        File.WriteAllText(Folder+"/remote-after.json",result.ToString());return result.ToString();
    }
    public static string RestoreSavedSource(){
        var avatar=CompletedTarget();var scene=avatar.scene;var initial=State()["sourceInspection"];
        var others=Enumerable.Range(0,SceneManager.sceneCount).Select(SceneManager.GetSceneAt).Where(s=>s.path!=scene.path).ToArray();
        if(others.Any(s=>s.path!="Packages/nadena.dev.ndmf/Runtime/___NDMF Preview___.unity"||s.isDirty||s.GetRootGameObjects().Any(g=>g.name!="nadena.dev.ndmf__Activator"&&(g.hideFlags&HideFlags.DontSave)==0)))throw new Exception("Additional user scenes are preserved");
        var allowed=new[]{"MF5 Gesture Manager","MF5 Camera","nadena.dev.ndmf__Activator","MorphForge_MF6_Avatar"};
        if(scene.GetRootGameObjects().Any(g=>!allowed.Contains(g.name)))throw new Exception("Unexpected scene objects are preserved");
        if(AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()!=(string)initial["prefabDependencyHash"])throw new Exception("Source prefab changed during upload; preserve it for review");
        bool dirty=scene.isDirty;const string snapshot="Assets/MorphForge/Validation/Upload20261022-AfterSdk.unity";
        if(dirty){
            if(AssetDatabase.LoadMainAssetAtPath(snapshot))throw new Exception("Post-SDK snapshot already exists; preserve current state");
            if(!AssetDatabase.IsValidFolder("Assets/MorphForge/Validation"))AssetDatabase.CreateFolder("Assets/MorphForge","Validation");
            if(!EditorSceneManager.SaveScene(scene,snapshot,true))throw new Exception("Could not preserve the post-SDK scene copy");
            // The source was clean when this task started. Preserve the SDK's
            // temporary preview/Activator state separately, then reopen its saved source.
            scene=EditorSceneManager.OpenScene(Mf6AvatarBuild.ScenePath,OpenSceneMode.Single);
        }
        var result=new JObject{["success"]=!scene.isDirty,["restoredSavedSource"]=dirty,["postSdkSnapshot"]=dirty?snapshot:null,["scene"]=scene.path,["sceneDirty"]=scene.isDirty,["sourcePrefabUnchanged"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString()==(string)initial["prefabDependencyHash"]};
        File.WriteAllText(Folder+"/scene-restoration.json",result.ToString());return result.ToString();
    }
}
