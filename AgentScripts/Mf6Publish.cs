using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading;
using System.Threading.Tasks;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;
using VRC.SDK3A.Editor;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf6Publish
{
    const string UserId="usr_c0dd064e-ac42-47a7-a17e-da6540382c55";
    const string AvatarName="MorphForge MF6 16K";
    static string StateFile=>"Docs/MF6/upload.json";
    static JObject State()=>File.Exists(StateFile)?JObject.Parse(File.ReadAllText(StateFile)):new JObject();
    static void Save(JObject s){Directory.CreateDirectory("Docs/MF6");File.WriteAllText(StateFile,s.ToString());}
    static GameObject Target(){
        if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
        var scene=SceneManager.GetSceneByPath(Mf6AvatarBuild.ScenePath);if(!scene.IsValid()||!scene.isLoaded)throw new Exception("MF6 scene must be open");
        var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
        if(avatar.name!="MorphForge_MF6_Avatar"||avatar.GetComponent<PipelineManager>().blueprintId!=Mf6AvatarBuild.AvatarId)throw new Exception("MF6 upload identity mismatch");
        return avatar;
    }
    public static string Preflight(){
        var avatar=Target();foreach(var file in new[]{"capacity","unified-smoke","random-smoke","native-visual","final-budget","prepared"}){
            var proof=JObject.Parse(File.ReadAllText("Docs/MF6/"+file+".json"));if((bool?)proof["success"]!=true)throw new Exception("Missing native proof: "+file);
        }
        var stores=avatar.GetComponentsInChildren<ParticleSystem>(true).Where(p=>p.name=="Store").ToArray();
        if(stores.Length!=8||stores.Any(p=>p.main.maxParticles!=2056||p.GetComponent<ParticleSystemRenderer>().sharedMaterial.GetFloat("_WireEncoding")!=6))throw new Exception("MF6 store mismatch");
        var budget=JObject.Parse(File.ReadAllText("Docs/MF6/final-budget.json"));if((int)budget["totalBits"]!=254)throw new Exception("Budget mismatch");
        var catalog=JObject.Parse(File.ReadAllText("Docs/MF6/avatar-catalog.json"));if(((JArray)catalog["materials"]).Count<500)throw new Exception("Expanded builtin catalog missing");
        var result=new JObject{["success"]=true,["targetAvatarId"]=Mf6AvatarBuild.AvatarId,["scene"]=avatar.scene.path,["stores"]=stores.Length,["editableCapacity"]=16384,["physicalCapacity"]=16448,["catalogStyles"]=((JArray)catalog["materials"]).Count,["dependencyHash"]=AssetDatabase.GetAssetDependencyHash(Mf6AvatarBuild.AvatarPath).ToString(),["timeUtc"]=DateTime.UtcNow.ToString("O")};File.WriteAllText("Docs/MF6/preflight.json",result.ToString());return result.ToString();
    }
    static string Thumbnail(GameObject avatar){
        var camera=avatar.scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<Camera>(true)).FirstOrDefault(c=>c.CompareTag("MainCamera"));if(!camera)throw new Exception("Scene MainCamera missing");
        string root=Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),"tmp","morphforge","mf6");Directory.CreateDirectory(root);string path=Path.Combine(root,"avatar-thumbnail.png");
        var previous=camera.targetTexture;var active=RenderTexture.active;var rt=RenderTexture.GetTemporary(512,512,24,RenderTextureFormat.ARGB32,RenderTextureReadWrite.sRGB);Texture2D image=null;
        try{camera.targetTexture=rt;camera.Render();RenderTexture.active=rt;image=new Texture2D(512,512,TextureFormat.RGB24,false);image.ReadPixels(new Rect(0,0,512,512),0,0);image.Apply();File.WriteAllBytes(path,image.EncodeToPNG());return path;}
        finally{camera.targetTexture=previous;RenderTexture.active=active;RenderTexture.ReleaseTemporary(rt);if(image)UnityEngine.Object.DestroyImmediate(image);}
    }
    static void PersistIdentity(GameObject avatar){
        string id=avatar.GetComponent<PipelineManager>().blueprintId;if(string.IsNullOrEmpty(id))return;
        var asset=PrefabUtility.LoadPrefabContents(Mf6AvatarBuild.AvatarPath);try{asset.GetComponent<PipelineManager>().blueprintId=id;PrefabUtility.SaveAsPrefabAsset(asset,Mf6AvatarBuild.AvatarPath);}finally{PrefabUtility.UnloadPrefabContents(asset);}
        EditorSceneManager.MarkSceneDirty(avatar.scene);EditorSceneManager.SaveScene(avatar.scene);AssetDatabase.SaveAssets();
        var catalog=JObject.Parse(File.ReadAllText("Docs/MF6/avatar-catalog.json"));catalog["targetAvatarId"]=id;File.WriteAllText("Docs/MF6/avatar-catalog.json",catalog.ToString());
    }
    public static string Status()=>State().ToString();
    public static async Task<string> Start(){
        var avatar=Target();Preflight();if((string)State()["status"]=="running")throw new Exception("MF6 upload already running");
        if(APIUser.CurrentUser==null&&ApiCredentials.Load())APIUser.InitialFetchCurrentUser(_=>{},null);for(int i=0;i<60&&APIUser.CurrentUser==null;i++)await Task.Delay(250);
        var user=APIUser.CurrentUser;if(user==null||user.id!=UserId||!user.canPublishAvatars)throw new Exception("Expected SDK publisher account");
        VRCAvatar remote;bool create=string.IsNullOrEmpty(Mf6AvatarBuild.AvatarId);
        if(create)remote=new VRCAvatar{Name=AvatarName,Description="MorphForge unified 240-bit geometry; native 16K boxes, textured slopes and RGB polygon faces. Standalone MF6 test avatar.",Tags=new List<string>(),ReleaseStatus="private"};
        else{remote=await VRCApi.GetAvatar(Mf6AvatarBuild.AvatarId,forceRefresh:true);if(remote.AuthorId!=UserId||remote.ReleaseStatus!="private"||remote.PendingUpload)throw new Exception("Expected idle owned private MF6 Avatar");remote.Name=AvatarName;}
        if(!VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi builder)){EditorWindow.GetWindow<VRCSdkControlPanel>(false,"VRChat SDK",true).Show();if(!VRCSdkControlPanel.TryGetBuilder(out builder))throw new Exception("SDK builder unavailable");}
        if(builder.BuildState!=SdkBuildState.Idle||builder.UploadState!=SdkUploadState.Idle)throw new Exception("SDK busy");
        string thumbnail=create?Thumbnail(avatar):null;string operation=Guid.NewGuid().ToString("N");Save(new JObject{["status"]="running",["operation"]=operation,["createIndependentPrivateAvatar"]=create,["targetAvatarId"]=Mf6AvatarBuild.AvatarId,["startedUtc"]=DateTime.UtcNow.ToString("O")});
        EditorApplication.CallbackFunction start=null;start=()=>{EditorApplication.update-=start;Upload(builder,avatar,remote,thumbnail,operation);};EditorApplication.update+=start;return State().ToString();
    }
    static async void Upload(IVRCSdkAvatarBuilderApi builder,GameObject avatar,VRCAvatar remote,string thumbnail,string operation){
        try{await builder.BuildAndUpload(avatar,remote,thumbnail,CancellationToken.None);PersistIdentity(avatar);var latest=await VRCApi.GetAvatar(Mf6AvatarBuild.AvatarId,forceRefresh:true);var s=State();s["status"]="success";s["targetAvatarId"]=latest.ID;s["avatarVersion"]=latest.Version;s["pendingUpload"]=latest.PendingUpload;s["completedUtc"]=DateTime.UtcNow.ToString("O");Save(s);}
        catch(Exception e){PersistIdentity(avatar);var s=State();s["status"]="failed";s["targetAvatarId"]=Mf6AvatarBuild.AvatarId;s["error"]=e.Message;s["completedUtc"]=DateTime.UtcNow.ToString("O");Save(s);Debug.LogError("MF6 upload failed: "+e.Message);}
    }
}
