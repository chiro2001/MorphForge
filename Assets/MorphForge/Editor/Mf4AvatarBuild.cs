using System;
using System.IO;
using System.Linq;
using MorphForge;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase.Editor.BuildPipeline;
using BlackStartX.GestureManager;
using nadena.dev.modular_avatar.core;

namespace MorphForge.Editor
{
    public static class Mf4AvatarBuild
    {
        public const string Root="Assets/MorphForge/MF4";
        public const string AvatarPath=Root+"/Avatar.prefab";
        public const string ScenePath=Root+"/MorphForge_MF4.unity";
        public const string AvatarId="avtr_df2ff222-b6ff-47fb-80be-e060868596a2";
        static string Hash(string path) { using(var h=System.Security.Cryptography.SHA256.Create())return BitConverter.ToString(h.ComputeHash(File.ReadAllBytes(path))).Replace("-","").ToLowerInvariant(); }
        public static string Prepare(string sourceRoot)
        {
            if(EditorApplication.isPlaying||EditorApplication.isCompiling)throw new Exception("Idle Edit Mode required.");
            for(int i=0;i<3;i++)Mf3DisplayBuild.BuildWideLane(i);
            var catalog=JObject.Parse(CatalogMaterialBuild.BuildAt(sourceRoot,"Assets/MorphForge/Diagnostics/MF4Catalog"));
            var material=AssetDatabase.LoadAssetAtPath<Material>("Assets/MorphForge/Diagnostics/MF4Catalog/ParticleBoxCatalog.mat");
            material.SetFloat("_WireEncoding",4);EditorUtility.SetDirty(material);AssetDatabase.SaveAssetIfDirty(material);
            // Original FT assets and dirty scenes stay untouched. Remove the FT
            // plugin injection objects only from this independent product copy.
            var contents=PrefabUtility.LoadPrefabContents(FullFTBuild.TestAvatarPath);
            try
            {
                contents.name="MorphForge_MF4_Avatar";
                contents.GetComponent<VRC.Core.PipelineManager>().blueprintId=AvatarId;
                foreach(var t in contents.transform.Cast<Transform>().ToArray())
                    if(t.name=="VRCFT_UE_Nemesis_VRCFur"||t.name=="MixExpressions"||t.name=="MorphForgePlacement")UnityEngine.Object.DestroyImmediate(t.gameObject);
                var placement=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(FullFTBuild.PlacementPath),contents.transform);
                placement.name="MorphForgePlacement";
                var center=placement.GetComponentsInChildren<Transform>(true).Single(t=>t.name=="ProtocolCenter");
                foreach(var child in center.Cast<Transform>().ToArray())if(child.GetComponent<ModularAvatarMergeAnimator>())UnityEngine.Object.DestroyImmediate(child.gameObject);
                for(int i=0;i<3;i++)
                {
                    var lane=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>(Root+"/Lane"+i+"/Display.prefab"),center);
                    lane.name="Lane"+i;
                    var store=lane.transform.Find("Store").GetComponent<ParticleSystem>();
                    var renderer=store.GetComponent<ParticleSystemRenderer>();renderer.sharedMaterial=material;
                    renderer.sortMode=ParticleSystemSortMode.Distance;
                    var settings=new SerializedObject(renderer);settings.FindProperty("m_ApplyActiveColorSpace").boolValue=false;settings.ApplyModifiedPropertiesWithoutUndo();
                }
                FullFTBuild.SanitizePickupParticleForces(contents);
                if(contents.GetComponentsInChildren<Transform>(true).Sum(t=>GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject))!=0)throw new Exception("Missing script on MF4 avatar.");
                PrefabUtility.SaveAsPrefabAsset(contents,AvatarPath);
            }
            finally { PrefabUtility.UnloadPrefabContents(contents); }
            AssetDatabase.SaveAssets();
            var source=AssetDatabase.LoadAssetAtPath<GameObject>(AvatarPath);
            var clone=UnityEngine.Object.Instantiate(source);clone.name="MorphForge_MF4_Build";clone.hideFlags=HideFlags.HideAndDontSave;
            JObject budget;
            try {
                if(!VRCBuildPipelineCallbacks.OnPreprocessAvatar(clone))throw new Exception("MF4 joint preprocess failed.");
                budget=JObject.Parse(File.ReadAllText("Docs/MF4/final-budget.json"));
                var descriptor=clone.GetComponent<VRCAvatarDescriptor>();
                var fx=descriptor.baseAnimationLayers.First(l=>l.type==VRCAvatarDescriptor.AnimLayerType.FX).animatorController;
                budget["fxLayers"]=((UnityEditor.Animations.AnimatorController)fx).layers.Length;
                budget["fxParameters"]=((UnityEditor.Animations.AnimatorController)fx).parameters.Length;
            } finally { UnityEngine.Object.DestroyImmediate(clone); }
            var previous=SceneManager.GetActiveScene();
            if(SceneManager.GetSceneByPath(ScenePath).IsValid())throw new Exception("MF4 scene already open; inspect before replacing.");
            var scene=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Additive);
            try
            {
                SceneManager.SetActiveScene(scene);
                var avatar=(GameObject)PrefabUtility.InstantiatePrefab(source,scene);avatar.name="MorphForge_MF4_Avatar";
                var gm=(GameObject)PrefabUtility.InstantiatePrefab(AssetDatabase.LoadAssetAtPath<GameObject>("Packages/vrchat.blackstartx.gesture-manager/GestureManager.prefab"),scene);
                gm.tag="EditorOnly";gm.name="MF4 Gesture Manager";var manager=gm.GetComponent<GestureManager>();manager.settings.favourite=avatar.GetComponent<VRCAvatarDescriptor>();manager.settings.loadStored=false;manager.settings.isRemote=false;
                var receiver=gm.AddComponent<MorphForgePlayReceiver>();receiver.ReceivePort=9012;receiver.SendPort=9013;
                var camera=new GameObject("MF4 Camera").AddComponent<Camera>();camera.tag="MainCamera";camera.transform.position=new Vector3(3,2.5f,4);camera.transform.LookAt(new Vector3(0,1,0));camera.nearClipPlane=.01f;camera.backgroundColor=new Color(.04f,.06f,.09f);camera.clearFlags=CameraClearFlags.SolidColor;camera.gameObject.AddComponent<AudioListener>();
                EditorSceneManager.SaveScene(scene,ScenePath);Selection.activeGameObject=avatar;
            }
            catch { EditorSceneManager.CloseScene(scene,true);SceneManager.SetActiveScene(previous);throw; }
            var report=new JObject{["success"]=true,["catalog"]=catalog,["budget"]=budget,["avatarId"]=AvatarId,["scene"]=ScenePath,["sourceSceneDirtyPreserved"]=previous.isDirty,["prefabSha256"]=Hash(AvatarPath),["activeCapacity"]=16384,["poolCapacity"]=16386,["lanes"]=3};
            Directory.CreateDirectory("Docs/MF4");
            var targetCatalog=JObject.Parse(File.ReadAllText(Path.Combine(sourceRoot,"catalog.json")));
            var capabilities=(JArray)targetCatalog["capabilities"]??new JArray();
            if(!capabilities.Any(v=>(string)v=="mf4-three-lane/1"))capabilities.Add("mf4-three-lane/1");
            targetCatalog["capabilities"]=capabilities;
            targetCatalog["targetAvatarId"]=AvatarId;
            targetCatalog["targetProtocol"]="mf4";
            File.WriteAllText("Docs/MF4/avatar-catalog.json",targetCatalog.ToString());
            File.WriteAllText("Docs/MF4/prepared.json",report.ToString());return report.ToString();
        }
    }
}
