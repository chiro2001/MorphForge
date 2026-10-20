using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.Dynamics;
using VRC.SDK3.Dynamics.Contact.Components;
using nadena.dev.modular_avatar.core;
using Newtonsoft.Json;

namespace MorphForge.Editor
{
    /// <summary>Explicit palm contacts make self pickup independent of another
    /// avatar's built-in contact setup. The visible handle remains active while held.</summary>
    public static class PlacementInteractionBuild
    {
        public const string ControllerPath = Mf6AvatarBuild.Root + "/Placement.controller";
        public static void Apply(GameObject avatar)
        {
            var module = avatar.transform.Find(PlacementStage4Build.ModuleName);
            if (!module) throw new Exception("MorphForge placement module missing");
            var merge = module.GetComponent<ModularAvatarMergeAnimator>();
            if (AssetDatabase.GetAssetPath(merge.animator) != ControllerPath)
            {
                if (!AssetDatabase.LoadAssetAtPath<AnimatorController>(ControllerPath) &&
                    !AssetDatabase.CopyAsset(AssetDatabase.GetAssetPath(merge.animator), ControllerPath))
                    throw new Exception("Cannot isolate MF6 placement controller");
                merge.animator = AssetDatabase.LoadAssetAtPath<AnimatorController>(ControllerPath);
                EditorUtility.SetDirty(merge);
            }
            var anchor = module.Find(PlacementStage4Build.AnchorPath);
            foreach (var side in new[] { new {name="Left",tag="MF6_HandL",parameter="MF_NearL"}, new {name="Right",tag="MF6_HandR",parameter="MF_NearR"} })
            {
                var hand=module.Find("HandReferences/" + side.name + "Hand");
                if (!hand) throw new Exception("Hand reference missing: " + side.name);
                var senderTransform=hand.Find("MorphForgeGripSender");
                if (!senderTransform) {senderTransform=new GameObject("MorphForgeGripSender").transform;senderTransform.SetParent(hand,false);}
                var sender=senderTransform.GetComponent<VRCContactSender>();
                if (!sender) sender=senderTransform.gameObject.AddComponent<VRCContactSender>();
                sender.rootTransform=hand;sender.shapeType=ContactBase.ShapeType.Sphere;sender.radius=.075f;
                sender.position=Vector3.zero;sender.collisionTags=new List<string>{side.tag};
                EditorUtility.SetDirty(sender);
                var receiver=module.GetComponentsInChildren<VRCContactReceiver>(true).Single(c=>c.parameter==side.parameter);
                receiver.rootTransform=anchor;receiver.shapeType=ContactBase.ShapeType.Sphere;receiver.radius=.16f;
                receiver.allowSelf=true;receiver.allowOthers=false;receiver.localOnly=true;
                receiver.receiverType=ContactReceiver.ReceiverType.Constant;
                receiver.collisionTags=new List<string>{side.tag};
                EditorUtility.SetDirty(receiver);
            }
            // Visibility controls only the handle, never its hand references. No
            // grab state writes HandleVisual; the same orange sphere stays visible.
            var visual=anchor.Find("HandleVisual");
            if (!visual) throw new Exception("Placement handle missing");
            EditorUtility.SetDirty(module.gameObject);
        }
        public static string ApplyCurrent()
        {
            var scene=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
            if (EditorApplication.isPlaying || scene.path!=Mf6AvatarBuild.ScenePath || scene.isDirty)
                throw new Exception("Requires the clean saved single-Avatar MF6 scene in Edit Mode");
            var avatar=scene.GetRootGameObjects().SelectMany(g=>g.GetComponentsInChildren<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>(true)).Single();
            var prefab=PrefabUtility.LoadPrefabContents(Mf6AvatarBuild.AvatarPath);
            try {Apply(prefab);PrefabUtility.SaveAsPrefabAsset(prefab,Mf6AvatarBuild.AvatarPath);}
            finally {PrefabUtility.UnloadPrefabContents(prefab);}
            Apply(avatar.gameObject);
            EditorSceneManager.MarkSceneDirty(scene);EditorSceneManager.SaveScene(scene);AssetDatabase.SaveAssets();
            var report=JsonConvert.SerializeObject(new{success=true,scene=scene.path,controller=ControllerPath,explicitHandSenders=2,receiverRadius=.16f,visibleDuringGrab=true,additionalSyncedBits=0,additionalAnimatorLayers=0},Formatting.Indented);
            Directory.CreateDirectory("Docs/StreamV16");File.WriteAllText("Docs/StreamV16/placement.json",report);return report;
        }
    }
}
