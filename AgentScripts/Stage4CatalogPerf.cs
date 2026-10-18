using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using UnityEditor;
using UnityEngine;

/// <summary>
/// Temporary native display benchmark for the installed catalog material.
/// This is not OSC evidence and does not alter the Avatar Store.
/// </summary>
public static class Stage4CatalogPerf
{
    const int Count = 16384;
    const string Evidence = "Docs/Materials/native";
    const string ReportPath = "Docs/Materials/catalog-16k-performance.json";

    public static async Task<string> Run()
    {
        if (!EditorApplication.isPlaying)
            throw new InvalidOperationException("Stage4CatalogPerf requires Editor Play Mode.");
        var material = AssetDatabase.LoadAssetAtPath<Material>(
            MorphForge.Editor.CatalogMaterialBuild.MaterialAssetPath);
        if (!material || !material.shader || material.shader.name != "MorphForge/ParticleBoxCatalog")
            throw new InvalidOperationException("Missing built MorphForge/ParticleBoxCatalog material.");
        var cube = CreateCubeMesh();
        var root = new GameObject("MorphForge 16k catalog performance fixture (NOT OSC)");
        root.layer = 31;
        var cameraObject = new GameObject("16k fixture camera", typeof(Camera));
        var lightObject = new GameObject("16k fixture light", typeof(Light));
        cameraObject.transform.SetParent(root.transform, false);
        lightObject.transform.SetParent(root.transform, false);
        var camera = cameraObject.GetComponent<Camera>();
        var light = lightObject.GetComponent<Light>();
        var oldAmbient = RenderSettings.ambientLight;
        var oldFog = RenderSettings.fog;
        try
        {
            RenderSettings.ambientLight = new Color(.22f, .22f, .22f);
            RenderSettings.fog = false;
            camera.orthographic = true;
            camera.orthographicSize = 6.5f;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = Color.black;
            camera.cullingMask = 1 << 31;
            camera.allowMSAA = false;
            camera.allowHDR = false;
            camera.transform.position = new Vector3(0, 0, 18);
            camera.transform.LookAt(Vector3.zero, Vector3.up);
            light.type = LightType.Directional;
            light.intensity = 1.25f;
            light.color = Color.white;
            lightObject.transform.rotation = Quaternion.Euler(35f, -35f, 0f);

            var systemObject = new GameObject("Catalog 16k mixed particle rig");
            systemObject.layer = 31;
            systemObject.transform.SetParent(root.transform, false);
            var system = systemObject.AddComponent<ParticleSystem>();
            var main = system.main;
            main.playOnAwake = false;
            main.loop = false;
            main.duration = .1f;
            main.startLifetime = 100000f;
            main.startSpeed = 0;
            main.startSize3D = true;
            main.maxParticles = Count;
            main.simulationSpace = ParticleSystemSimulationSpace.World;
            main.scalingMode = ParticleSystemScalingMode.Hierarchy;
            var emission = system.emission;
            emission.enabled = false;
            var shape = system.shape;
            shape.enabled = false;
            var renderer = systemObject.GetComponent<ParticleSystemRenderer>();
            renderer.enabled = true;
            renderer.renderMode = ParticleSystemRenderMode.Mesh;
            renderer.mesh = cube;
            renderer.sharedMaterial = material;
            renderer.alignment = ParticleSystemRenderSpace.Local;
            renderer.enableGPUInstancing = false;
            renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            renderer.SetActiveVertexStreams(new List<ParticleSystemVertexStream>
            {
                ParticleSystemVertexStream.Position,
                ParticleSystemVertexStream.Normal,
                ParticleSystemVertexStream.Color,
                ParticleSystemVertexStream.UV,
                ParticleSystemVertexStream.UV2,
                ParticleSystemVertexStream.SizeXYZ
            });
            var serialized = new SerializedObject(renderer);
            var colorSpace = serialized.FindProperty("m_ApplyActiveColorSpace");
            if (colorSpace != null)
            {
                colorSpace.boolValue = false;
                serialized.ApplyModifiedPropertiesWithoutUndo();
            }
            system.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);

            var particles = new ParticleSystem.Particle[Count];
            for (var i = 0; i < Count; i++)
            {
                var x = i % 32;
                var y = (i / (32 * 32)) % 16;
                var z = (i / 32) % 32;
                var style = (i % 5) switch
                {
                    0 => 10, // metal opaque
                    1 => 11, // metal opaque
                    2 => 16, // water transparent
                    3 => 19, // cloud opaque
                    _ => 21  // cutout
                };
                particles[i] = new ParticleSystem.Particle
                {
                    position = new Vector3((x * 3 - 48) * .25f, y * 3 * .25f, (z * 3 - 48) * .25f),
                    startSize3D = Vector3.one * .25f,
                    startLifetime = 100000f,
                    remainingLifetime = 100000f,
                    startColor = new Color32(
                        (byte)Mathf.RoundToInt((style & 63) * 255f / 63f),
                        (byte)Mathf.RoundToInt(((style >> 6) & 63) * 255f / 63f),
                        0,
                        0),
                    randomSeed = (uint)(i + 1)
                };
            }
            system.SetParticles(particles, particles.Length);
            system.Simulate(0f, true, false, false);
            system.Play(false);
            system.Pause(false);
            await Task.Delay(750);

            var samples = new List<double>();
            var lastFrame = -1;
            var end = Time.realtimeSinceStartupAsDouble + 6;
            while (Time.realtimeSinceStartupAsDouble < end)
            {
                if (Time.frameCount != lastFrame)
                {
                    lastFrame = Time.frameCount;
                    samples.Add(Time.unscaledDeltaTime * 1000.0);
                }
                await Task.Delay(5);
            }
            samples.Sort();
            var p50 = samples[(int)((samples.Count - 1) * .5)];
            var p95 = samples[(int)((samples.Count - 1) * .95)];
            var max = samples.Last();
            var pixels = Render(camera, 1280, 720);
            SavePng("catalog-16k-performance.png", pixels, 1280, 720);
            var visible = renderer.isVisible;
            var count = system.particleCount;
            var report = new
            {
                success = count == Count && visible && pixels.Count(pixel => Mathf.Max(pixel.r, Mathf.Max(pixel.g, pixel.b)) > 3) > 100,
                scope = "Temporary native ParticleSystem display fixture; NOT OSC, NOT avatar Store, NOT a multiplayer/client benchmark",
                count,
                capacity = Count,
                visible,
                mixedStyles = new[] { 10, 11, 16, 19, 21 },
                mixedKinds = new[] { "metal", "metal", "water", "cloud", "cutout" },
                samples = samples.Count,
                p50Ms = p50,
                p95Ms = p95,
                maxMs = max,
                frameSeconds = 6,
                image = Evidence + "/catalog-16k-performance.png",
                rendererBounds = new
                {
                    center = new[] { renderer.bounds.center.x, renderer.bounds.center.y, renderer.bounds.center.z },
                    size = new[] { renderer.bounds.size.x, renderer.bounds.size.y, renderer.bounds.size.z }
                },
                note = "VRChat client GPU cost, per-particle sorting and user filtering remain separate."
            };
            WriteJson(report);
            return Newtonsoft.Json.JsonConvert.SerializeObject(report, Newtonsoft.Json.Formatting.Indented);
        }
        finally
        {
            RenderSettings.ambientLight = oldAmbient;
            RenderSettings.fog = oldFog;
            UnityEngine.Object.DestroyImmediate(root);
            UnityEngine.Object.DestroyImmediate(cube);
        }
    }

    static Mesh CreateCubeMesh()
    {
        var primitive = GameObject.CreatePrimitive(PrimitiveType.Cube);
        var mesh = UnityEngine.Object.Instantiate(primitive.GetComponent<MeshFilter>().sharedMesh);
        mesh.name = "MorphForge 16k fixture cube";
        var vertices = mesh.vertices;
        var normals = mesh.normals;
        var tileUV = new Vector2[vertices.Length];
        var faceUV = new Vector2[vertices.Length];
        for (var i = 0; i < vertices.Length; i++)
        {
            var vertex = vertices[i];
            var normal = normals[i];
            var axis = Mathf.Abs(normal.x) > .5f ? 0 : (Mathf.Abs(normal.y) > .5f ? 1 : 2);
            tileUV[i] = axis == 0 ? new Vector2(vertex.z + .5f, vertex.y + .5f)
                : axis == 1 ? new Vector2(vertex.x + .5f, vertex.z + .5f)
                : new Vector2(vertex.x + .5f, vertex.y + .5f);
            faceUV[i] = new Vector2(axis, 0);
        }
        mesh.uv = tileUV;
        mesh.uv2 = faceUV;
        UnityEngine.Object.DestroyImmediate(primitive);
        return mesh;
    }

    static Color32[] Render(Camera camera, int width, int height)
    {
        var target = RenderTexture.GetTemporary(width, height, 24, RenderTextureFormat.ARGB32);
        var previous = RenderTexture.active;
        var texture = new Texture2D(width, height, TextureFormat.RGB24, false);
        try
        {
            camera.targetTexture = target;
            camera.aspect = (float)width / height;
            camera.Render();
            RenderTexture.active = target;
            texture.ReadPixels(new Rect(0, 0, width, height), 0, 0);
            texture.Apply();
            return texture.GetPixels32();
        }
        finally
        {
            camera.targetTexture = null;
            RenderTexture.active = previous;
            RenderTexture.ReleaseTemporary(target);
            UnityEngine.Object.DestroyImmediate(texture);
        }
    }

    static void SavePng(string relative, Color32[] pixels, int width, int height)
    {
        var texture = new Texture2D(width, height, TextureFormat.RGB24, false);
        try
        {
            texture.SetPixels32(pixels);
            texture.Apply();
            var path = Path.GetFullPath(Path.Combine(Application.dataPath, "..", Evidence, relative));
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            File.WriteAllBytes(path, texture.EncodeToPNG());
        }
        finally
        {
            UnityEngine.Object.DestroyImmediate(texture);
        }
    }

    static void WriteJson(object value)
    {
        var path = Path.GetFullPath(Path.Combine(Application.dataPath, "..", ReportPath));
        Directory.CreateDirectory(Path.GetDirectoryName(path));
        File.WriteAllText(path, Newtonsoft.Json.JsonConvert.SerializeObject(value, Newtonsoft.Json.Formatting.Indented) + Environment.NewLine);
    }
}
