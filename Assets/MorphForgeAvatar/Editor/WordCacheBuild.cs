using System;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEditor.Animations;
using UnityEngine;
using VRC.SDK3.Avatars.Components;
using VRC.SDKBase;

namespace MorphForgeAvatar.Editor {
    // State motions latch exact 16-bit floats directly into local Animator
    // parameters. A ParameterDriver cannot copy animation-driven parameters;
    // rendering trees consume these retained values without a driver copy.
    public static class WordCacheBuild {
        public struct Input { public string target; public int firstByte; }
        static AnimatorController owner;
        static Dictionary<string, Motion> motions;
        static string Normalized(int index)=>"MF6_WN"+index.ToString("D2");
        static string Complement(int index)=>"MF6_WC"+index.ToString("D2");
        public static void AddInputs(AnimatorController controller) {
            for(int i=0;i<29;i++)foreach(bool complement in new[]{false,true}) {
                string name=complement?Complement(i):Normalized(i);
                if(!controller.parameters.Any(p=>p.name==name))controller.AddParameter(new AnimatorControllerParameter{name=name,type=AnimatorControllerParameterType.Float,defaultFloat=complement?1:0});
            }
        }
        public static void ExtendSnapshot(AnimatorController controller) {
            AddInputs(controller);
            var inbox=controller.layers.Single(l=>l.name=="MF6 240bit atomic local snapshot").stateMachine;
            foreach(var state in inbox.states.Select(s=>s.state).Where(s=>s.name.StartsWith("Text snapshot ")||s.name.StartsWith("Extension "))) {
                var driver=state.behaviours.OfType<VRCAvatarParameterDriver>().Single();
                driver.parameters.RemoveAll(p=>p.name.StartsWith("MF6_WN")||p.name.StartsWith("MF6_WC"));
                for(int i=0;i<29;i++)foreach(bool complement in new[]{false,true})driver.parameters.Add(new VRC_AvatarParameterDriver.Parameter {
                    name=complement?Complement(i):Normalized(i),source="MF6_D"+i.ToString("D2"),type=VRC_AvatarParameterDriver.ChangeType.Copy,
                    convertRange=true,sourceMin=0,sourceMax=255,destMin=complement?1:0,destMax=complement?0:1});
                EditorUtility.SetDirty(driver);
            }
            EditorUtility.SetDirty(controller);
        }
        public static Input Pair(string target, int firstByte) {
            if (firstByte < 0 || firstByte > 27) throw new ArgumentOutOfRangeException(nameof(firstByte));
            return new Input { target = target, firstByte = firstByte };
        }
        public static void Begin(AnimatorController controller) {
            owner = controller; motions = new Dictionary<string, Motion>(); AddInputs(controller);
        }
        // Retain packed AAP storage independently of material output. Empty
        // panels and inactive animation banks can stop writing renderer curves.
        public static Motion Hold(string one, IEnumerable<string> parameters) {
            var tree=new BlendTree{name="Retained word storage",blendType=BlendTreeType.Direct};
            AssetDatabase.AddObjectToAsset(tree,owner);
            var names=parameters.Distinct().ToArray();
            foreach(string name in names) {
                var clip=new AnimationClip{name="Retain "+name,frameRate=60};
                AnimationUtility.SetEditorCurve(clip,EditorCurveBinding.FloatCurve("",typeof(Animator),name),AnimationCurve.Constant(0,.02f,1));
                AssetDatabase.AddObjectToAsset(clip,owner);tree.AddChild(clip);
            }
            var children=tree.children;for(int i=0;i<children.Length;i++)children[i].directBlendParameter=names[i];tree.children=children;
            var serialized=new SerializedObject(tree);serialized.FindProperty("m_NormalizedBlendValues").boolValue=false;serialized.ApplyModifiedPropertiesWithoutUndo();return tree;
        }
        public static Motion Receive(string one, IEnumerable<Input> input) {
            if (!owner) throw new InvalidOperationException("Begin a cache controller build first.");
            var words = input.ToArray(); string key = one + ":" + string.Join(";", words.Select(w => w.target + "@" + w.firstByte));
            if (motions.TryGetValue(key, out var existing)) return existing;
            var tree = new BlendTree { name = "Latch " + words[0].target, blendType = BlendTreeType.Direct };
            AssetDatabase.AddObjectToAsset(tree, owner);
            // Explicit zero curves overwrite old values when both bytes are
            // zero. Write Defaults Off preserves unrelated pages on exit.
            var zero = new AnimationClip { name = "Cache zero " + words[0].target, frameRate = 60 };
            foreach (var word in words) AnimationUtility.SetEditorCurve(zero,
                EditorCurveBinding.FloatCurve("", typeof(Animator), word.target), AnimationCurve.Constant(0, .02f, 0));
            AssetDatabase.AddObjectToAsset(zero, owner);
            var weights = new List<string>();
            foreach (var word in words) for (int part = 0; part < 2; part++) {
                // Each byte contributes normalized + complement = 1. The
                // total weight is 2*N, so unrelated retained pose values are
                // preserved and explicit zero curves prevent old-word residue.
                var clip=new AnimationClip{name="Cache "+word.target+" coefficient "+part,frameRate=60};
                float coefficient=words.Length*2*255*(part==0?1:256);
                foreach(var output in words)AnimationUtility.SetEditorCurve(clip,
                    EditorCurveBinding.FloatCurve("",typeof(Animator),output.target),
                    AnimationCurve.Constant(0,.02f,output.target==word.target?coefficient:0));
                AssetDatabase.AddObjectToAsset(clip,owner);tree.AddChild(clip);weights.Add(Normalized(word.firstByte+part));
                tree.AddChild(zero);weights.Add(Complement(word.firstByte+part));
            }
            var children = tree.children;
            for (int i = 0; i < children.Length; i++) children[i].directBlendParameter = weights[i];
            tree.children = children;
            var serialized = new SerializedObject(tree); serialized.FindProperty("m_NormalizedBlendValues").boolValue = true;
            serialized.ApplyModifiedPropertiesWithoutUndo(); motions.Add(key, tree); return tree;
        }
    }
}
