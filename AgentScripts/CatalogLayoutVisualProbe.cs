using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEngine;
using Object = UnityEngine.Object;

/// <summary>Actual mesh-particle rendering of the full metadata layout, without Play Mode or OSC.</summary>
public static class CatalogLayoutVisualProbe
{
    const string OriginalPath = "Assets/MorphForge/MaterialsCatalog/ParticleBoxCatalog.mat";
    const string GridPath = "Assets/MorphForge/Diagnostics/CatalogSecurity/CatalogGrid.mat";
    static readonly Type Fixture = typeof(MorphForge.Editor.Stage4FinishBuild);

    static object Call(string name, params object[] args) =>
        Fixture.GetMethod(name, BindingFlags.NonPublic | BindingFlags.Static).Invoke(null, args);

    public static string Run() => RunVisual(GridPath, OriginalPath,
        "Docs/Security/catalog-grid-visual", "all catalog styles match original layout");

    public static string RunProduction() => RunVisual(OriginalPath, GridPath,
        "Docs/Security/catalog-production-visual", "production matches server-accepted diagnostic reference");

    static string RunVisual(string subjectPath, string referencePath, string evidenceRoot, string comparisonName)
    {
        if (EditorApplication.isPlaying || EditorApplication.isCompiling)
            throw new InvalidOperationException("Requires idle Edit Mode.");
        var original = AssetDatabase.LoadAssetAtPath<Material>(referencePath);
        var grid = AssetDatabase.LoadAssetAtPath<Material>(subjectPath);
        if (!original || !grid) throw new InvalidOperationException("Missing probe materials.");
        Directory.CreateDirectory(evidenceRoot);
        var oldAmbient = RenderSettings.ambientLight;
        var oldFog = RenderSettings.fog;
        var root = new GameObject("Temporary Catalog Grid visual probe") { layer = 31, hideFlags = HideFlags.DontSave };
        var mesh = (Mesh)Call("CreateCubeMesh");
        var garbage = new List<Object>();
        var checks = new List<object>();
        try
        {
            RenderSettings.ambientLight = new Color(.22f, .22f, .22f);
            RenderSettings.fog = false;
            var cameraGo = new GameObject("Probe camera", typeof(Camera));
            cameraGo.transform.SetParent(root.transform, false);
            var camera = cameraGo.GetComponent<Camera>();
            camera.enabled = false;
            camera.orthographic = true;
            camera.nearClipPlane = .01f;
            camera.farClipPlane = 100f;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = Color.black;
            camera.cullingMask = 1 << 31;
            camera.allowMSAA = false;
            camera.allowHDR = false;
            var lightGo = new GameObject("Probe light", typeof(Light));
            lightGo.transform.SetParent(root.transform, false);
            lightGo.transform.rotation = Quaternion.Euler(35f, -35f, 0f);
            lightGo.GetComponent<Light>().intensity = 1.25f;
            lightGo.GetComponent<Light>().cullingMask = 1 << 31;
            var go = (GameObject)Call("CreateParticleSystem", "Probe particles", root.transform, grid, mesh, ParticleSystemSimulationSpace.Local);
            var system = go.GetComponent<ParticleSystem>();
            var renderer = go.GetComponent<ParticleSystemRenderer>();
            var main = system.main;
            main.maxParticles = 512;
            var metadata = (Texture2D)original.GetTexture("_CatalogMeta");
            var manifest = JObject.Parse(File.ReadAllText("Docs/Materials/catalog.json"));
            var ids = manifest["materials"].Select(m => (int)m["id"]).OrderBy(id => id).ToArray();
            var particles = ids.Select((id, i) => Particle(new Vector3((i % 12) * .85f - 4.675f, (i / 12) * .85f - 3.4f, 0), new Vector3(.65f, .65f, .65f), id, 0)).ToArray();
            Commit(system, particles);
            camera.orthographicSize = 4.5f;
            camera.transform.position = new Vector3(0, 0, 12);
            camera.transform.LookAt(Vector3.zero);
            renderer.sharedMaterial = original;
            var oldPixels = Render(camera, 1200, 900);
            renderer.sharedMaterial = grid;
            var newPixels = Render(camera, 1200, 900);
            checks.Add(Difference(comparisonName, oldPixels, newPixels));
            Save(evidenceRoot, "catalog-original.png", oldPixels, 1200, 900);
            Save(evidenceRoot, "catalog-grid.png", newPixels, 1200, 900);

            // The established fixtures assert expected face colors and animation frames.
            var faces = JObject.FromObject(Call("CaptureSixFaces", camera, system, evidenceRoot + "/six-face"));
            foreach (var face in faces["value"])
                face["file"] = evidenceRoot + "/" + Path.GetFileName((string)face["file"]);
            checks.AddRange(faces["value"].Select(v => (object)v));
            var animation = JObject.FromObject(Call("CaptureAnimationFrames", camera, system, evidenceRoot + "/animation.png"));
            animation["value"]["file"] = evidenceRoot + "/animation.png";
            checks.Add(animation["value"]);
            var visibility = JObject.FromObject(Call("CaptureStyleVisibility", camera, system, evidenceRoot + "/styles.png"));
            visibility["value"]["file"] = evidenceRoot + "/styles.png";
            checks.Add(visibility["value"]);

            // Exercise each row boundary and high material IDs with distinct synthetic colors.
            var specialIds = new[] { 63, 64, 1023, 1024, 4095 };
            var colors = new[] { Color.red, Color.green, Color.blue, Color.yellow, Color.magenta };
            var data = metadata.GetRawTextureData();
            var floats = new float[data.Length / 4];
            Buffer.BlockCopy(data, 0, floats, 0, data.Length);
            for (var i = 0; i < specialIds.Length; i++)
            {
                var index = specialIds[i] * 16;
                for (var j = 0; j < 16; j++) floats[index + j] = 0;
                floats[index + 4] = colors[i].r;
                floats[index + 5] = colors[i].g;
                floats[index + 6] = colors[i].b;
                floats[index + 7] = 1;
                floats[index + 9] = 1;
                floats[index + 10] = .4f;
            }
            var synthetic = new Texture2D(256, 64, TextureFormat.RGBAFloat, false, true)
                { filterMode = FilterMode.Point, wrapMode = TextureWrapMode.Clamp };
            garbage.Add(synthetic);
            synthetic.SetPixelData(floats, 0);
            synthetic.Apply(false, false);
            var testMaterial = new Material(grid);
            garbage.Add(testMaterial);
            testMaterial.SetTexture("_CatalogMeta", synthetic);
            renderer.sharedMaterial = testMaterial;
            Commit(system, specialIds.Select((id, i) => Particle(new Vector3(i - 2f, 0, 0), Vector3.one * .72f, id, 0)).ToArray());
            camera.orthographicSize = 1.1f;
            camera.transform.position = new Vector3(0, 0, 8);
            camera.transform.LookAt(Vector3.zero);
            var boundaryPixels = Render(camera, 1400, 420);
            Save(evidenceRoot, "boundary-styles.png", boundaryPixels, 1400, 420);
            for (var i = 0; i < specialIds.Length; i++)
            {
                var point = camera.WorldToViewportPoint(new Vector3(i - 2f, 0, 0));
                var observed = (Color)Call("Average", boundaryPixels, 1400, 420,
                    Mathf.RoundToInt(point.x * 1400), Mathf.RoundToInt(point.y * 420), 8);
                var norm = new Vector3(observed.r, observed.g, observed.b).normalized;
                var expected = new Vector3(colors[i].r, colors[i].g, colors[i].b).normalized;
                checks.Add(new { name = "high style " + specialIds[i], pass = Vector3.Dot(norm, expected) > .98f,
                    rgb = new[] { observed.r, observed.g, observed.b } });
            }

            // Root transform/scale equivalence is checked against the pre-change shader.
            renderer.sharedMaterial = grid;
            Commit(system, new[] { Particle(Vector3.zero, Vector3.one * .72f, 1, 0) });
            camera.orthographicSize = .8f;
            camera.transform.position = new Vector3(0, 0, 4);
            camera.transform.LookAt(Vector3.zero);
            var baseline = Render(camera, 512, 512);
            go.transform.localScale = Vector3.one * .5f;
            camera.orthographicSize *= .5f;
            camera.transform.position *= .5f;
            var scaled = Render(camera, 512, 512);
            checks.Add(Difference("half-scale local texture", baseline, scaled, .06));
            Save(evidenceRoot, "scale-half.png", scaled, 512, 512);
            camera.transform.SetParent(go.transform, true);
            lightGo.transform.SetParent(go.transform, true);
            var rigidBefore = Render(camera, 512, 512);
            go.transform.SetPositionAndRotation(new Vector3(3, 1.5f, -2), Quaternion.Euler(17, 37, -9));
            var rigidAfter = Render(camera, 512, 512);
            checks.Add(Difference("rigid root keeps local texture", rigidBefore, rigidAfter, .01));
            Save(evidenceRoot, "rigid-before.png", rigidBefore, 512, 512);
            Save(evidenceRoot, "rigid-after.png", rigidAfter, 512, 512);

            var report = new { success = checks.All(c => (bool)JObject.FromObject(c)["pass"]),
                generatedAtUtc = DateTime.UtcNow.ToString("O"), checks, catalogIds = ids,
                material = subjectPath, referenceMaterial = referencePath, metadataSize = new[] { 256, 64 }, fullStyles = 4096,
                scope = "Actual native mesh particles + Camera.Render; idle Edit Mode; not a VRChat client capture." };
            var json = JsonConvert.SerializeObject(report, Formatting.Indented);
            File.WriteAllText(evidenceRoot + "/report.json", json + Environment.NewLine);
            return json;
        }
        finally
        {
            RenderSettings.ambientLight = oldAmbient;
            RenderSettings.fog = oldFog;
            Object.DestroyImmediate(root);
            Object.DestroyImmediate(mesh);
            foreach (var item in garbage) Object.DestroyImmediate(item);
        }
    }

    static ParticleSystem.Particle Particle(Vector3 position, Vector3 size, int style, int motion) =>
        (ParticleSystem.Particle)Call("Particle", position, size, style, 0, motion);
    static void Commit(ParticleSystem system, ParticleSystem.Particle[] particles) =>
        Call("CommitParticles", system, particles, particles.Length);
    static Color32[] Render(Camera camera, int width, int height) => (Color32[])Call("Render", camera, width, height);
    static void Save(string evidenceRoot, string file, Color32[] pixels, int width, int height) => Call("SavePng", evidenceRoot + "/" + file, pixels, width, height);
    static object Difference(string name, Color32[] first, Color32[] second, double threshold = .001)
    {
        var error = Convert.ToDouble(Call("MeanAbsoluteError", first, second));
        var visible = first.Count(c => c.r + c.g + c.b > 30);
        return new { name, pass = error < threshold && visible > 100, meanAbsoluteError = error, threshold, visiblePixels = visible };
    }
}
