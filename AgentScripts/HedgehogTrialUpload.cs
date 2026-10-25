using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Text.RegularExpressions;
using System.Threading;
using System.Threading.Tasks;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.Core;
using VRC.SDK3.Avatars.Components;
using VRC.SDK3A.Editor;
using VRC.SDKBase.Editor;
using VRC.SDKBase.Editor.Api;
using MorphForgeAvatar;

// Explicit upload entry only: no InitializeOnLoad, request watcher, consent
// cache changes or automatic copyright-dialog acceptance.
public static class HedgehogTrialUpload {
    const string Project="D:/Projects/VRChatProjects2/MorphForgeHedgehogDemo_1_4_0";
    const string ScenePath="Assets/MorphForgeHedgehogTrial/MorphForgeHedgehogTrial.unity";
    const string PrefabPath="Assets/MorphForgeHedgehogTrial/MorphForgeHedgehogTrial.prefab";
    static string StatePath=>"TrialReview/upload-state.json";
    static JObject State()=>File.Exists(StatePath)?JObject.Parse(File.ReadAllText(StatePath)):new JObject();
    static void Save(JObject state){Directory.CreateDirectory("TrialReview");state["updatedUtc"]=DateTime.UtcNow.ToString("O");File.WriteAllText(StatePath,state.ToString());}
    static string Message(string s)=>Regex.Replace(s??"",@"https?://\S+","[request URL omitted]");
    static GameObject Target(){
        if(!Application.dataPath.Replace('\\','/').Equals(Project+"/Assets",StringComparison.OrdinalIgnoreCase))throw new Exception("Dedicated MorphForge trial project required");
        if(EditorApplication.isPlaying||EditorApplication.isCompiling||SceneManager.GetActiveScene().path!=ScenePath||SceneManager.GetActiveScene().isDirty)throw new Exception("Saved trial scene in ready Edit Mode required");
        var avatar=SceneManager.GetActiveScene().GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single().gameObject;
        if(avatar.name!="MorphForge 1.4.0 - 256Hedgehog Trial"||!avatar.GetComponentInChildren<MorphForgeAvatarModule>(true))throw new Exception("Unexpected trial avatar");
        var validation=JObject.Parse(File.ReadAllText("TrialReview/validation.json"));
        var saved=State();var id=avatar.GetComponent<PipelineManager>().blueprintId;
        string prefabHash=AssetDatabase.GetAssetDependencyHash(PrefabPath).ToString(),sceneHash=AssetDatabase.GetAssetDependencyHash(ScenePath).ToString();
        // The SDK assigns an ID to the scene even when an upload is interrupted.
        // Accept only the exact scene saved by this uploader, with the same review.
        bool reviewedScene=(string)validation["validatedSceneHash"]==sceneHash;
        bool recordedScene=!string.IsNullOrEmpty(id)&&id==(string)saved["avatarId"]&&sceneHash==(string)saved["savedSceneHash"]&&(string)saved["reviewSceneHash"]==(string)validation["validatedSceneHash"];
        if((string)validation["status"]!="passed"||(string)validation["validatedPrefabHash"]!=prefabHash||(!reviewedScene&&!recordedScene))throw new Exception("Trial changed since native SDK validation; revalidate it");
        return avatar;
    }
    public static string Inspect(){
        var avatar=Target();return new JObject{["sourceReady"]=true,["sdkLoggedIn"]=APIUser.IsLoggedIn,["canPublish"]=APIUser.CurrentUser?.canPublishAvatars??false,["avatarIdAssigned"]=!string.IsNullOrEmpty(avatar.GetComponent<PipelineManager>().blueprintId),["menuLanguage"]=avatar.GetComponentInChildren<MorphForgeAvatarModule>(true).menuLanguage,["thumbnailPresent"]=File.Exists("TrialReview/hedgehog-thumbnail.png"),["uploaded"]=(bool?)State()["uploaded"]??false}.ToString();
    }
    public static string OpenSdk(){
        var avatar=Target();Selection.activeGameObject=avatar;
        EditorWindow.GetWindow<VRCSdkControlPanel>(false,"VRChat SDK",true).Show();
        return "SDK window opened for the prepared trial; no upload started";
    }
    // Invoke only after the user has approved publishing this concrete trial.
    public static string BeginPublicUpload(){
        var avatar=Target();
        if(!APIUser.IsLoggedIn||!(APIUser.CurrentUser?.canPublishAvatars??false))throw new Exception("Sign in through the SDK Account tab with avatar publishing permission");
        if(!VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi builder))throw new Exception("Open the SDK Builder tab first");
        if(builder.BuildState!=SdkBuildState.Idle||builder.UploadState!=SdkUploadState.Idle)throw new Exception("SDK operation already in progress; preserve it");
        var previous=State();var id=avatar.GetComponent<PipelineManager>().blueprintId;
        if((string)previous["status"]=="running")throw new Exception("Trial upload is already queued or running; inspect its status before retrying");
        if(!string.IsNullOrEmpty(id)&&id!=(string)previous["avatarId"])throw new Exception("Existing ID is not this trial's recorded upload; never overwrite another avatar");
        var review=JObject.Parse(File.ReadAllText("TrialReview/validation.json"));
        var operation=Guid.NewGuid().ToString("N");Save(new JObject{["status"]="running",["phase"]="queued",["operation"]=operation,["avatarId"]=id,["intendedReleaseStatus"]="public",["uploaded"]=false,["reviewSceneHash"]=review["validatedSceneHash"],["savedSceneHash"]=previous["savedSceneHash"]});
        builder.SelectAvatar(avatar);
        EditorApplication.CallbackFunction next=null;next=()=>{EditorApplication.update-=next;Upload(builder,avatar,operation);};EditorApplication.update+=next;EditorApplication.QueuePlayerLoopUpdate();return State().ToString();
    }
    static void Progress(string phase,string message,float? fraction=null){var state=State();state["phase"]=phase;state["message"]=Message(message);state["progress"]=fraction;Save(state);}
    static async void Upload(IVRCSdkAvatarBuilderApi builder,GameObject avatar,string operation){
        EventHandler<string> onBuild=(o,text)=>Progress("build",text);
        EventHandler<(string status,float percentage)> onUpload=(o,p)=>Progress("upload",p.status,p.percentage);
        builder.OnSdkBuildProgress+=onBuild;builder.OnSdkUploadProgress+=onUpload;
        try{
            var metadataFile=JObject.Parse(File.ReadAllText("TrialReview/upload-metadata.json"));var pipeline=avatar.GetComponent<PipelineManager>();
            var metadata=string.IsNullOrEmpty(pipeline.blueprintId)?new VRCAvatar():await VRCApi.GetAvatar(pipeline.blueprintId,forceRefresh:true);
            if(!string.IsNullOrEmpty(metadata.ID)&&metadata.AuthorId!=APIUser.CurrentUser.id)throw new Exception("Trial ID is owned by another SDK user");
            metadata.Name=(string)metadataFile["name"];metadata.Description=(string)metadataFile["description"];metadata.ReleaseStatus="public";metadata.Tags=metadata.Tags??new List<string>();
            Progress("sdk-invoked","Official SDK upload of the new public trial; any copyright agreement stays in the SDK UI");
            await builder.BuildAndUpload(avatar,metadata,Path.GetFullPath("TrialReview/hedgehog-thumbnail.png"),CancellationToken.None);
            var remote=await VRCApi.GetAvatar(pipeline.blueprintId,forceRefresh:true);
            if(remote.AuthorId!=APIUser.CurrentUser.id||remote.ReleaseStatus!="public"||remote.PendingUpload||remote.Name!=metadata.Name)throw new Exception("Uploaded trial metadata not confirmed");
            var state=State();if((string)state["operation"]!=operation)throw new Exception("Upload operation state changed");
            state["status"]="success";state["phase"]="complete";state["avatarId"]=remote.ID;state["releaseStatus"]=remote.ReleaseStatus;state["pendingUpload"]=remote.PendingUpload;state["version"]=remote.Version;state["name"]=remote.Name;state["uploaded"]=true;state["ownedBySdkUser"]=true;state["publicTrialAvatarUrl"]="https://vrchat.com/home/avatar/"+remote.ID;Save(state);
            File.WriteAllText("TrialReview/uploaded-avatar.json",state.ToString());
        }catch(Exception e){var state=State();state["status"]="failed";state["error"]=Message(e.Message);Save(state);Debug.LogError("MorphForge trial upload failed: "+Message(e.Message));}
        finally{
            builder.OnSdkBuildProgress-=onBuild;builder.OnSdkUploadProgress-=onUpload;
            // Retain an ID reserved by SDK even on failure, preventing duplicate
            // records on retry. Only this scene is ever saved.
            if(avatar&&avatar.GetComponent<PipelineManager>()){
                var pipeline=avatar.GetComponent<PipelineManager>();var state=State();state["avatarId"]=pipeline.blueprintId;Save(state);
                EditorUtility.SetDirty(pipeline);PrefabUtility.RecordPrefabInstancePropertyModifications(pipeline);EditorSceneManager.SaveScene(avatar.scene,ScenePath);AssetDatabase.SaveAssets();
                state["savedSceneHash"]=AssetDatabase.GetAssetDependencyHash(ScenePath).ToString();Save(state);
                if((bool?)state["uploaded"]==true)File.WriteAllText("TrialReview/uploaded-avatar.json",state.ToString());
            }
        }
    }
    public static string Status()=>State().ToString();
}
