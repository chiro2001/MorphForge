using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
using BlackStartX.GestureManager;
using nadena.dev.modular_avatar.core;
using Newtonsoft.Json.Linq;
namespace MorphForge.Editor
{
    public static class Mf6AvatarBuild
    {
        public const string Root=Mf6NativeBuild.Root;
        public const string AvatarPath=Root+"/Avatar.prefab";
        public const string ScenePath=Root+"/MorphForge_MF6.unity";
        public static string AvatarId{get{var asset=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarPath);return asset?asset.GetComponent<VRC.Core.PipelineManager>().blueprintId:"";}}
        public static string Prepare(string sourceRoot=null)
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required");
            for(int i=0;i<SceneManager.sceneCount;i++)if(SceneManager.GetSceneAt(i).isDirty)throw new Exception("Unsaved scene preserved: "+SceneManager.GetSceneAt(i).path);
            string id=AvatarId,materialSource=null;
            if(!string.IsNullOrEmpty(sourceRoot)){CatalogMaterialBuild.BuildAt(sourceRoot,"Assets/MorphForge/Diagnostics/MF6Catalog");materialSource="Assets/MorphForge/Diagnostics/MF6Catalog/ParticleBoxCatalog.mat";}
            var catalog=JObject.Parse(File.ReadAllText(string.IsNullOrEmpty(sourceRoot)?"Docs/MF5/avatar-catalog.json":Path.Combine(sourceRoot,"catalog.json")));
            catalog["capabilities"]=new JArray("mf6-text-panels/1","mf6-text-latest14/1","mf6-unified/1","mf6-region8/1","mf6-triangles/1","mf6-local-strip/1","mf6-shared-boxes/1","mf6-voxels8/1","mf5-runtime-materials/1","transparent-face-mask/1","virtual-light/1","scale-reference-0.05/1","texture-scale/1","texture-rotation/1","mf6-wedges/1");catalog["targetProtocol"]="mf6";catalog["targetAvatarId"]=id;
            for(int i=0;i<8;i++)Mf6NativeBuild.BuildWorker(i,materialSource);
            var decoder=JObject.Parse(Mf6DecoderBuild.Build());
            var contents=PrefabUtility.LoadPrefabContents(Mf5AvatarBuild.AvatarPath);
            try{
                contents.name="MorphForge_MF6_Avatar";contents.GetComponent<VRC.Core.PipelineManager>().blueprintId=id;
                var center=contents.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ProtocolCenter");foreach(var child in center.Cast<Transform>().ToArray())UnityEngine.Object.DestroyImmediate(child.gameObject);
                for(int i=0;i<8;i++){var worker=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/Pool"+i+"/Display.prefab"),center);worker.name="Pool"+i;}
                var manager=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/Decoder.prefab"),center);manager.name="MF6Decoder";
                PlacementInteractionBuild.Apply(contents);
                MorphForgeAvatar.Editor.PlacementControls.Apply(contents.transform.Find(PlacementStage4Build.ModuleName).gameObject);
                if(!AssetDatabase.LoadAssetAtPath<GameObject>(Mf6TextBuild.Root+"/TextDisplay.prefab"))Mf6TextBuild.BuildAssets();
                Mf6TextBuild.Install(contents.transform.Find(PlacementStage4Build.ModuleName).gameObject);
                catalog["identityHash"]=AvatarCapabilitiesBuild.Apply(contents,catalog);
                if(contents.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject))!=0)throw new Exception("Missing script on MF6 avatar");
                PrefabUtility.SaveAsPrefabAsset(contents,AvatarPath);
            }finally{PrefabUtility.UnloadPrefabContents(contents);}
            var source=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarPath);var clone=UnityEngine.Object.Instantiate(source);clone.name="MorphForge_MF6_Build";clone.hideFlags=HideFlags.HideAndDontSave;JObject budget;
            try{
                if(!VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone))throw new Exception("MF6 preprocess failed");budget=JObject.Parse(File.ReadAllText("Docs/MF6/final-budget.json"));
                var descriptor=clone.GetComponent<VRCAvatarDescriptor>();var fx=descriptor.baseAnimationLayers.First(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController as AnimatorController;budget["fxLayers"]=fx.layers.Length;budget["fxParameters"]=fx.parameters.Length;
            }finally{UnityEngine.Object.DestroyImmediate(clone);}
            var scene=EditorSceneManager.OpenScene(Mf5AvatarBuild.ScenePath,OpenSceneMode.Single);var old=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true)).Single();UnityEngine.Object.DestroyImmediate(old.gameObject);
            var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source,scene);avatar.name="MorphForge_MF6_Avatar";
            var gm=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<GestureManager>(true)).Single();gm.settings.favourite=avatar.GetComponent<VRCAvatarDescriptor>();gm.settings.loadStored=false;gm.settings.isRemote=false;gm.GetComponent<MorphForgePlayReceiver>().Avatar=avatar.GetComponent<VRCAvatarDescriptor>();
            EditorSceneManager.SaveScene(scene,ScenePath);AssetDatabase.SaveAssets();
            Directory.CreateDirectory("Docs/MF6");File.WriteAllText("Docs/MF6/avatar-catalog.json",catalog.ToString());
            var report=new JObject{["success"]=true,["scene"]=ScenePath,["avatarId"]=id,["capacity"]=16384,["pools"]=8,["decoder"]=decoder,["budget"]=budget,["sourceMf5Preserved"]=true,["avatarCount"]=scene.GetRootGameObjects().Sum(g=>g.GetComponentsInChildren<VRCAvatarDescriptor>(true).Length)};File.WriteAllText("Docs/MF6/prepared.json",report.ToString());return report.ToString();
        }
    }
}
