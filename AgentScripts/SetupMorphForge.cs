using System;
using System.IO;
using System.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;

public static class SetupMorphForge
{
    const string PrefabPath = "Assets/Nemesis/Nemesis.prefab";
    const string ScenePath = "Assets/MorphForge/Scenes/MorphForge_Nemesis.unity";
    const string ReportPath = "Docs/Setup/validation.json";

    [Serializable] public class Report
    {
        public bool success;
        public string checkedAt;
        public string projectPath;
        public string unityVersion;
        public string scene;
        public string prefab;
        public bool humanoid;
        public bool avatarDescriptor;
        public int missingScripts;
        public int renderers;
        public int skinnedMeshes;
        public string[] missingDependencies;
        public string[] shaderIssues;
        public string[] packages;
        public string preview;
    }

    public static string Run()
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Wait for Edit Mode and completed compilation.");

        var prefab = AssetDatabase.LoadAssetAtPath<GameObject>(PrefabPath);
        if (!prefab) throw new InvalidOperationException("Nemesis prefab did not import.");
        var animator = prefab.GetComponent<Animator>();
        bool humanoid = animator && animator.avatar && animator.avatar.isValid && animator.avatar.isHuman;
        bool descriptor = prefab.GetComponents<Component>().Any(c => c && c.GetType().FullName == "VRC.SDK3.Avatars.Components.VRCAvatarDescriptor");
        int missing = prefab.GetComponentsInChildren<Transform>(true)
            .Sum(t => GameObjectUtility.GetMonoBehavioursWithMissingScriptCount(t.gameObject));
        var renderers = prefab.GetComponentsInChildren<Renderer>(true);
        var materials = renderers.SelectMany(r => r.sharedMaterials).Where(m => m).Distinct().ToArray();
        var shaderIssues = materials.Where(m => !m.shader || !m.shader.isSupported || m.shader.name == "Hidden/InternalErrorShader")
            .Select(m => m.name + ": " + (m.shader ? m.shader.name : "missing shader")).ToArray();
        var missingDeps = AssetDatabase.GetDependencies(PrefabPath, true)
            .Where(p => (p.StartsWith("Assets/") || p.StartsWith("Packages/")) && !AssetDatabase.LoadMainAssetAtPath(p)).ToArray();
        if (!humanoid || !descriptor || missing != 0 || shaderIssues.Length != 0 || missingDeps.Length != 0)
            throw new InvalidOperationException("Nemesis validation failed: humanoid=" + humanoid + ", descriptor=" + descriptor + ", missingScripts=" + missing + ", shaderIssues=" + string.Join(";", shaderIssues) + ", dependencies=" + string.Join(";", missingDeps));

        Scene scene;
        if (File.Exists(ScenePath))
        {
            scene = SceneManager.GetSceneByPath(ScenePath);
            if (!scene.IsValid() || !scene.isLoaded)
            {
                EnsureNoDirtyScenes();
                scene = EditorSceneManager.OpenScene(ScenePath, OpenSceneMode.Single);
            }
        }
        else
        {
            EnsureNoDirtyScenes();
            EnsureFolder("Assets/MorphForge/Scenes");
            scene = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Single);
            var avatar = (GameObject)PrefabUtility.InstantiatePrefab(prefab, scene);
            avatar.name = "MorphForge_Nemesis";
            avatar.transform.position = Vector3.zero;

            // Keep the new test instance independent of any inherited upload ID.
            var pipeline = avatar.GetComponents<Component>().FirstOrDefault(c => c && c.GetType().FullName == "VRC.Core.PipelineManager");
            if (pipeline)
            {
                var serialized = new SerializedObject(pipeline);
                var blueprint = serialized.FindProperty("blueprintId");
                if (blueprint != null)
                {
                    blueprint.stringValue = string.Empty;
                    serialized.ApplyModifiedPropertiesWithoutUndo();
                    PrefabUtility.RecordPrefabInstancePropertyModifications(pipeline);
                }
            }

            var floor = GameObject.CreatePrimitive(PrimitiveType.Plane);
            floor.name = "Preview Floor";
            floor.transform.localScale = new Vector3(0.5f, 1f, 0.5f);
            var light = new GameObject("Preview Light").AddComponent<Light>();
            light.type = LightType.Directional;
            light.intensity = 1.1f;
            light.transform.rotation = Quaternion.Euler(45f, -30f, 0f);
            var camera = new GameObject("Main Camera").AddComponent<Camera>();
            camera.tag = "MainCamera";
            camera.transform.position = new Vector3(0f, 1.05f, 2.9f);
            camera.transform.LookAt(new Vector3(0f, 0.85f, 0f));
            camera.fieldOfView = 42f;
            camera.nearClipPlane = 0.03f;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = new Color(0.12f, 0.15f, 0.19f);
            if (!EditorSceneManager.SaveScene(scene, ScenePath))
                throw new InvalidOperationException("Cannot save the MorphForge baseline scene.");
            AssetDatabase.SaveAssets();
        }

        Directory.CreateDirectory("Docs/Setup");
        string preview = "Docs/Setup/morphforge-nemesis.png";
        var sceneCamera = scene.GetRootGameObjects().SelectMany(o => o.GetComponentsInChildren<Camera>(true)).FirstOrDefault();
        if (sceneCamera) Capture(sceneCamera, preview);
        var report = new Report {
            success = true,
            checkedAt = DateTimeOffset.Now.ToString("o"),
            projectPath = Path.GetDirectoryName(Application.dataPath),
            unityVersion = Application.unityVersion,
            scene = ScenePath,
            prefab = PrefabPath,
            humanoid = humanoid,
            avatarDescriptor = descriptor,
            missingScripts = missing,
            renderers = renderers.Length,
            skinnedMeshes = prefab.GetComponentsInChildren<SkinnedMeshRenderer>(true).Count(r => r.sharedMesh),
            missingDependencies = missingDeps,
            shaderIssues = shaderIssues,
            packages = UnityEditor.PackageManager.PackageInfo.GetAllRegisteredPackages()
                .Where(p => p.name == "com.unity.pipeline" || p.name.StartsWith("com.vrchat.") || p.name == "jp.lilxyzw.liltoon" || p.name.StartsWith("nadena.dev.") || p.name == "minminmean.supine" || p.name.Contains("gesture-manager"))
                .Select(p => p.name + "@" + p.version).OrderBy(p => p).ToArray(),
            preview = sceneCamera ? preview : null
        };
        var json = JsonUtility.ToJson(report, true);
        File.WriteAllText(ReportPath, json);
        Debug.Log("MorphForge setup validation passed: " + ReportPath);
        return json;
    }

    static void EnsureFolder(string path)
    {
        var parts = path.Split('/');
        string parent = parts[0];
        for (int i = 1; i < parts.Length; i++)
        {
            var next = parent + "/" + parts[i];
            if (!AssetDatabase.IsValidFolder(next)) AssetDatabase.CreateFolder(parent, parts[i]);
            parent = next;
        }
    }

    static void EnsureNoDirtyScenes()
    {
        for (int i = 0; i < SceneManager.sceneCount; i++)
            if (SceneManager.GetSceneAt(i).isDirty)
                throw new InvalidOperationException("An existing scene has unsaved edits; setup will not replace it.");
    }

    static void Capture(Camera camera, string path)
    {
        var oldTarget = camera.targetTexture;
        var oldActive = RenderTexture.active;
        var target = RenderTexture.GetTemporary(640, 640, 24, RenderTextureFormat.ARGB32);
        var pixels = new Texture2D(640, 640, TextureFormat.RGB24, false);
        try
        {
            camera.targetTexture = target;
            camera.Render();
            RenderTexture.active = target;
            pixels.ReadPixels(new Rect(0, 0, 640, 640), 0, 0);
            pixels.Apply();
            File.WriteAllBytes(path, pixels.EncodeToPNG());
        }
        finally
        {
            camera.targetTexture = oldTarget;
            RenderTexture.active = oldActive;
            RenderTexture.ReleaseTemporary(target);
            UnityEngine.Object.DestroyImmediate(pixels);
        }
    }
}
