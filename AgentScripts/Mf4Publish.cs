using System;
using System.IO;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;
using VRC.SDK3A.Editor;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf4Publish
{
    const string UserId="usr_c0dd064e-ac42-47a7-a17e-da6540382c55";
    static string StateFile => Path.Combine(Directory.GetParent(Application.dataPath).FullName,"Docs/MF4/upload.json");
    static JObject State() => File.Exists(StateFile)?JObject.Parse(File.ReadAllText(StateFile)):new JObject();
    static void Save(JObject s){Directory.CreateDirectory("Docs/MF4");File.WriteAllText(StateFile,s.ToString());}
    static GameObject Target()
    {
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required.");
        var scene=SceneManager.GetSceneByPath(Mf4AvatarBuild.ScenePath);
        if(!scene.IsValid()||!scene.isLoaded)throw new Exception("MF4 scene must be open.");
        var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
        if(avatar.name!="MorphForge_MF4_Avatar"||avatar.GetComponent<PipelineManager>().blueprintId!=Mf4AvatarBuild.AvatarId)throw new Exception("Upload identity mismatch.");
        return avatar;
    }
    public static string Preflight()
    {
        var avatar=Target();
        var smoke=JObject.Parse(File.ReadAllText("Docs/MF4/native-smoke.json"));var visual=JObject.Parse(File.ReadAllText("Docs/MF4/visual.json"));var budget=JObject.Parse(File.ReadAllText("Docs/MF4/final-budget.json"));
        if(!(bool)smoke["success"]||!(bool)visual["success"]||!(bool)budget["success"]||(int)budget["totalBits"]!=254)throw new Exception("Validation proof missing/failed.");
        var systems=avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").ToArray();
        if(systems.Length!=3||systems.Any(p=>p.main.maxParticles!=5462||!p.GetComponent<ParticleSystemRenderer>().sharedMaterial||p.GetComponent<ParticleSystemRenderer>().sharedMaterial.GetFloat("_WireEncoding")!=4))throw new Exception("MF4 native pool/material mismatch.");
        var report=new JObject{["success"]=true,["targetAvatarId"]=Mf4AvatarBuild.AvatarId,["scene"]=avatar.scene.path,["root"]=avatar.name,["dependencyHash"]=AssetDatabase.GetAssetDependencyHash(Mf4AvatarBuild.AvatarPath).ToString(),["timeUtc"]=DateTime.UtcNow.ToString("O"),["bits"]=254,["stores"]=3,["maxParticlesTotal"]=16386,["customColorsStable"]=true};
        File.WriteAllText("Docs/MF4/preflight.json",report.ToString());return report.ToString();
    }
    public static string Status() => State().ToString();
    public static async Task<string> Start()
    {
        var avatar=Target();Preflight();
        if(APIUser.CurrentUser==null&&ApiCredentials.Load())APIUser.InitialFetchCurrentUser(_=>{},null);
        for(int i=0;i<60&&APIUser.CurrentUser==null;i++)await Task.Delay(250);
        var user=APIUser.CurrentUser;if(user==null||user.id!=UserId||!user.canPublishAvatars)throw new Exception("SDK account does not match expected publisher.");
        var remote=await VRCApi.GetAvatar(Mf4AvatarBuild.AvatarId,forceRefresh:true);
        if(remote.AuthorId!=UserId||remote.ReleaseStatus!="private"||remote.PendingUpload)throw new Exception("Expected idle private Avatar owned by this account.");
        if(!VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi builder)){EditorWindow.GetWindow<VRCSdkControlPanel>(false,"VRChat SDK",true).Show();if(!VRCSdkControlPanel.TryGetBuilder(out builder))throw new Exception("SDK builder missing.");}
        if(builder.BuildState!=SdkBuildState.Idle||builder.UploadState!=SdkUploadState.Idle)throw new Exception("SDK is busy.");
        var operation=Guid.NewGuid().ToString("N");var state=State();if((string)state["status"]=="running")throw new Exception("Upload already running.");
        state=new JObject{["status"]="running",["operation"]=operation,["targetAvatarId"]=Mf4AvatarBuild.AvatarId,["versionBefore"]=remote.Version,["startedUtc"]=DateTime.UtcNow.ToString("O"),["displayName"]="MorphForge MF4 16K",["geometryBits"]=240,["totalBits"]=254};Save(state);
        remote.Name="MorphForge MF4 16K";
        // Pipeline auto-tick services update while an unfocused Editor may leave
        // delayCall queued indefinitely. Start once from the next real update.
        EditorApplication.CallbackFunction start=null;
        start=()=>{EditorApplication.update-=start;Upload(builder,avatar,remote,operation);};
        EditorApplication.update+=start;
        return new JObject{["accepted"]=true,["operation"]=operation,["targetAvatarId"]=Mf4AvatarBuild.AvatarId,["versionBefore"]=remote.Version}.ToString();
    }
    static async void Upload(IVRCSdkAvatarBuilderApi builder,GameObject avatar,VRCAvatar remote,string operation)
    {
        try {var initial=State();initial["phase"]="sdk-invoked";Save(initial);await builder.BuildAndUpload(avatar,remote,null,CancellationToken.None);var latest=await VRCApi.GetAvatar(Mf4AvatarBuild.AvatarId,forceRefresh:true);var s=State();if((string)s["operation"]!=operation)return;s["status"]="success";s["phase"]="complete";s["avatarVersion"]=latest.Version;s["pendingUpload"]=latest.PendingUpload;s["completedUtc"]=DateTime.UtcNow.ToString("O");Save(s);}
        catch(Exception e){var s=State();s["status"]="failed";s["error"]=e.ToString();s["completedUtc"]=DateTime.UtcNow.ToString("O");Save(s);Debug.LogError("MF4 upload failed: "+e);}
    }
}
