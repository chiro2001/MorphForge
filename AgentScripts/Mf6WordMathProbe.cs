using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.Animations;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

// Isolated animation arithmetic fixture. Does not edit any product assets.
public static class Mf6WordMathProbe
{
    public static string Latch()
    {
        var scene = EditorSceneManager.NewPreviewScene(); var allocated = new List<UnityEngine.Object>(); var rows = new JArray();
        try {
            var root = new GameObject("Word latch fixture"); SceneManager.MoveGameObjectToScene(root, scene);
            var animator = root.AddComponent<Animator>(); var fx = new AnimatorController(); allocated.Add(fx);
            fx.AddParameter("Active", AnimatorControllerParameterType.Bool); fx.AddParameter("Word", AnimatorControllerParameterType.Float); fx.AddLayer("Latch");
            var machine = fx.layers[0].stateMachine; allocated.Add(machine);
            var clip = new AnimationClip(); allocated.Add(clip);
            AnimationUtility.SetEditorCurve(clip, EditorCurveBinding.FloatCurve("", typeof(Animator), "Word"), AnimationCurve.Constant(0, 1, 12345));
            var active = machine.AddState("Active"); allocated.Add(active); active.motion = clip; active.writeDefaultValues = false;
            var idle = machine.AddState("Idle"); allocated.Add(idle); idle.writeDefaultValues = false; machine.defaultState = idle;
            var t = idle.AddTransition(active); allocated.Add(t); t.hasExitTime = false; t.duration = 0; t.AddCondition(AnimatorConditionMode.If, 0, "Active");
            t = active.AddTransition(idle); allocated.Add(t); t.hasExitTime = false; t.duration = 0; t.AddCondition(AnimatorConditionMode.IfNot, 0, "Active");
            animator.runtimeAnimatorController = fx; animator.cullingMode = AnimatorCullingMode.AlwaysAnimate; animator.Rebind();
            foreach (bool enabled in new[] { false, true, true, false, false, false }) {
                animator.SetBool("Active", enabled); animator.Update(1f / 60);
                rows.Add(new JObject { ["enabled"] = enabled, ["word"] = animator.GetFloat("Word"), ["curveControlled"] = animator.IsParameterControlledByCurve("Word") });
            }
        } finally { EditorSceneManager.ClosePreviewScene(scene); foreach (var value in allocated.AsEnumerable().Reverse()) if (value) UnityEngine.Object.DestroyImmediate(value); }
        return new JObject { ["rows"] = rows }.ToString();
    }
    public static string Run()
    {
        if (EditorApplication.isPlaying) throw new Exception("Edit Mode required.");
        var scene = EditorSceneManager.NewPreviewScene();
        var allocated = new List<UnityEngine.Object>();
        var checks = new JArray();
        try
        {
            var root = new GameObject("MF6 word math fixture");
            SceneManager.MoveGameObjectToScene(root, scene);
            var animator = root.AddComponent<Animator>();
            var fx = new AnimatorController { name = "MF6 word math fixture" }; allocated.Add(fx);
            foreach (var name in new[] { "Low", "High", "Word" }) fx.AddParameter(name, AnimatorControllerParameterType.Float);
            fx.AddLayer("Word arithmetic"); var machine = fx.layers[0].stateMachine; allocated.Add(machine);
            var tree = new BlendTree { name = "Low + 256 High", blendType = BlendTreeType.Direct }; allocated.Add(tree);
            for (int i = 0; i < 2; i++)
            {
                var clip = new AnimationClip { name = "Word coefficient " + i }; allocated.Add(clip);
                AnimationUtility.SetEditorCurve(clip, EditorCurveBinding.FloatCurve("", typeof(Animator), "Word"),
                    AnimationCurve.Constant(0, 1, i == 0 ? 1 : 256));
                tree.AddChild(clip);
            }
            var children = tree.children; children[0].directBlendParameter = "Low"; children[1].directBlendParameter = "High";
            tree.children = children;
            var serialized = new SerializedObject(tree); serialized.FindProperty("m_NormalizedBlendValues").boolValue = false;
            serialized.ApplyModifiedPropertiesWithoutUndo();
            var state = machine.AddState("Decode"); allocated.Add(state); state.motion = tree; state.writeDefaultValues = true; machine.defaultState = state;
            animator.cullingMode = AnimatorCullingMode.AlwaysAnimate; animator.runtimeAnimatorController = fx; animator.Rebind();
            foreach (var pair in new[] { new[] { 0, 0 }, new[] { 255, 0 }, new[] { 0, 1 }, new[] { 255, 255 }, new[] { 0, 0 }, new[] { 45, 78 }, new[] { 0, 78 }, new[] { 0, 0 } })
            {
                animator.SetFloat("Low", pair[0]); animator.SetFloat("High", pair[1]);
                for (int tick = 0; tick < 3; tick++) animator.Update(1f / 60);
                float actual = animator.GetFloat("Word"); int expected = pair[0] + pair[1] * 256;
                checks.Add(new JObject { ["low"] = pair[0], ["high"] = pair[1], ["expected"] = expected,
                    ["actual"] = actual, ["pass"] = Mathf.Abs(actual - expected) < .01f });
            }
        }
        finally
        {
            EditorSceneManager.ClosePreviewScene(scene);
            foreach (var value in allocated.AsEnumerable().Reverse()) if (value) UnityEngine.Object.DestroyImmediate(value);
        }
        var result = new JObject { ["success"] = checks.All(c => (bool)c["pass"]), ["checks"] = checks,
            ["scope"] = "Transient preview scene only; native Animator float arithmetic, no Avatar or product asset edits." };
        Directory.CreateDirectory("Docs/Security20261022");
        File.WriteAllText("Docs/Security20261022/word-math.json", result.ToString()); return result.ToString();
    }
}
