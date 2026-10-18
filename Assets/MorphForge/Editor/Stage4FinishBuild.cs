using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using Newtonsoft.Json.Linq;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using UnityEngine.SceneManagement;
using Object = UnityEngine.Object;

namespace MorphForge.Editor
{
    /// <summary>
    /// Post-long-test integration for the native catalog material. This is an
    /// Editor-only build/validation entry point; the generated Avatar contains
    /// no MorphForge runtime script from this class.
    /// </summary>
    public static class Stage4FinishBuild
    {
        public const string GeometryPrefabPath = Mf3DisplayBuild.Root + "/Display.prefab";
        public const string PlacementPrefabPath = PlacementStage4Build.Root + "/Placement.prefab";
        public const string Stage4AvatarPath = "Assets/MorphForge/Stage4/Avatar.prefab";
        public const string FullFTAvatarPath = "Assets/MorphForge/FullFT/Avatar.prefab";
        public const string ValidationScenePath = "Assets/MorphForge/Scenes/MorphForge_MaterialsValidation.unity";
        public const string MaterialCatalogPath = "Docs/Materials/catalog.json";
        public const string FinishReportPath = "Docs/Materials/native-finish.json";
        public const string VisualReportPath = "Docs/Materials/native-visual-validation.json";
        public const string VisualEvidenceDirectory = "Docs/Materials/native";
        public const string ModuleName = "MorphForgeGeometry";
        public const int FixtureAnimation = 25;
        public const int FixtureSixFace = 26;

        static readonly string[] RequiredStreams =
        {
            "Position", "Normal", "Color", "UV", "UV2", "SizeXYZ"
        };

        [MenuItem("Tools/MorphForge/Stage 4/Finish materials integration")]
        static void MenuFinish()
        {
            Debug.Log(Finish());
        }

        [MenuItem("Tools/MorphForge/Stage 4/Inspect materials integration")]
        static void MenuInspect()
        {
            Debug.Log(Inspect());
        }

        [MenuItem("Tools/MorphForge/Stage 4/Validate visual fixture")]
        static void MenuValidateVisual()
        {
            Debug.Log(ValidateVisual());
        }

        /// <summary>
        /// Run once after the long Play test has ended. It rebuilds geometry,
        /// rebuilds the catalog material, binds that material and the 0.5s pump
        /// lifetime to product prefabs, and prepares an independent validation
        /// scene. It deliberately does not enter Play Mode.
        /// </summary>
        public static string Finish()
        {
            RequireIdleEditMode();
            var started = DateTime.UtcNow;
            var beforeScenes = SnapshotScenes();
            var geometryBuild = JObject.Parse(Mf3DisplayBuild.Build());
            var catalogBuild = JObject.Parse(CatalogMaterialBuild.Build());
            var catalog = ReadCatalog();
            var fixture = FindFixture(catalog);
            var material = AssetDatabase.LoadAssetAtPath<Material>(CatalogMaterialBuild.MaterialAssetPath);
            if (!material)
                throw new InvalidOperationException("Catalog material asset was not created.");
            if (!material.shader || material.shader.name != "MorphForge/ParticleBoxCatalog")
                throw new InvalidOperationException("Catalog material does not use MorphForge/ParticleBoxCatalog.");

            var fire = FindFireClip();
            if (fire == null || Mathf.Abs(fire.length - .03f) > .001f)
                throw new InvalidOperationException("MF3 Fire clip is missing or is not 0.03 seconds.");

            var prefabResults = new List<object>
            {
                BindPrefab(GeometryPrefabPath, material, true),
                BindPrefab(PlacementPrefabPath, material, true),
                BindPrefab(Stage4AvatarPath, material, true)
            };
            if (AssetDatabase.LoadAssetAtPath<GameObject>(FullFTAvatarPath))
                prefabResults.Add(BindPrefab(FullFTAvatarPath, material, false));
            AssetDatabase.SaveAssets();

            var sceneResult = ApplyToActiveScene(material, beforeScenes);
            var fixtureScene = BuildFixtureScene();

            var report = new
            {
                success = true,
                phase = "edit-mode-finish",
                startedUtc = started.ToString("O"),
                finishedUtc = DateTime.UtcNow.ToString("O"),
                sceneGate = "Call only after the long Play test ends.",
                geometry = geometryBuild,
                catalog = catalogBuild,
                materialManifest = new
                {
                    path = MaterialCatalogPath,
                    count = catalog.materials.Count,
                    ids = catalog.materials.Select(item => item.id).OrderBy(id => id).ToArray(),
                    fixture25 = fixture.animation,
                    fixture26 = fixture.sixFace
                },
                material = new
                {
                    path = CatalogMaterialBuild.MaterialAssetPath,
                    shader = material.shader.name,
                    atlas = CatalogMaterialBuild.AtlasAssetPath,
                    metadata = CatalogMaterialBuild.MetadataAssetPath,
                    cellSize = material.GetFloat("_CellSize")
                },
                fireClipSeconds = fire.length,
                prefabs = prefabResults,
                activeScene = sceneResult,
                fixtureScene,
                beforeScenes,
                playValidation = new
                {
                    completed = false,
                    command = "Enter Play Mode, then run Stage4FinishEntry.ValidateVisual",
                    evidence = "Pending actual native ParticleSystem rendering and pixel classification."
                },
                unsavedScenePolicy = "A scene dirty before Finish is never saved or modified. A clean single-Avatar scene is updated only inside MorphForgeGeometry."
            };
            WriteJson(FinishReportPath, report);
            return JsonConvert.SerializeObject(report, Formatting.Indented);
        }

        /// <summary>
        /// Read-only integration check. Safe to run before or after the visual
        /// validation; it does not call a builder or save an asset.
        /// </summary>
        public static string Inspect()
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                throw new InvalidOperationException("Inspect requires idle Edit Mode.");
            var catalog = ReadCatalog();
            var fixture = FindFixture(catalog);
            var material = AssetDatabase.LoadAssetAtPath<Material>(CatalogMaterialBuild.MaterialAssetPath);
            var fire = FindFireClip();
            var prefabs = new[]
            {
                InspectPrefab(GeometryPrefabPath, true),
                InspectPrefab(PlacementPrefabPath, true),
                InspectPrefab(Stage4AvatarPath, true),
                InspectPrefab(FullFTAvatarPath, false)
            };
            var report = new
            {
                success = material != null && fire != null &&
                          Mathf.Abs(fire.length - .03f) < .001f &&
                          prefabs.Where(item => item.required).All(item => item.present && item.success),
                mode = "read-only",
                catalogCount = catalog.materials.Count,
                fixture,
                material = material == null ? null : new
                {
                    path = CatalogMaterialBuild.MaterialAssetPath,
                    shader = material.shader ? material.shader.name : null,
                    cellSize = material.GetFloat("_CellSize")
                },
                fireClipSeconds = fire == null ? (float?)null : fire.length,
                prefabs
            };
            WriteJson("Docs/Materials/native-inspect.json", report);
            return JsonConvert.SerializeObject(report, Formatting.Indented);
        }

        /// <summary>
        /// Must be invoked from Editor Play Mode. It creates a temporary native
        /// ParticleSystem rig, renders catalog pixels, writes PNG evidence, and
        /// removes the rig. It is not part of the Avatar or a product build.
        /// </summary>
        public static string ValidateVisual()
        {
            if (!EditorApplication.isPlaying)
                throw new InvalidOperationException("Enter Play Mode before running the visual fixture.");
            var material = AssetDatabase.LoadAssetAtPath<Material>(CatalogMaterialBuild.MaterialAssetPath);
            if (!material)
                throw new InvalidOperationException("Missing built CatalogMaterialBuild.ParticleBoxCatalog material.");

            Directory.CreateDirectory(FindProjectPath(VisualEvidenceDirectory));
            var oldAmbient = RenderSettings.ambientLight;
            var oldFog = RenderSettings.fog;
            var oldTimeScale = Time.timeScale;
            var root = new GameObject("MorphForge native material validation rig");
            root.layer = 31;
            var cameraObject = new GameObject("Native fixture camera", typeof(Camera));
            var lightObject = new GameObject("Native fixture directional light", typeof(Light));
            cameraObject.transform.SetParent(root.transform, false);
            lightObject.transform.SetParent(root.transform, false);
            var camera = cameraObject.GetComponent<Camera>();
            var light = lightObject.GetComponent<Light>();
            var cube = CreateCubeMesh();
            var checks = new List<object>();
            var captures = new List<string>();
            try
            {
                RenderSettings.ambientLight = new Color(.22f, .22f, .22f);
                RenderSettings.fog = false;
                Time.timeScale = 1f;
                camera.enabled = true;
                camera.orthographic = true;
                camera.nearClipPlane = .01f;
                camera.farClipPlane = 50f;
                camera.clearFlags = CameraClearFlags.SolidColor;
                camera.backgroundColor = Color.black;
                camera.cullingMask = 1 << 31;
                camera.allowMSAA = false;
                camera.allowHDR = false;
                light.type = LightType.Directional;
                light.intensity = 1.25f;
                light.color = Color.white;
                lightObject.transform.rotation = Quaternion.Euler(35f, -35f, 0f);

                var animation = CreateParticleSystem("fixture25 animation", root.transform, material, cube);
                var animationSystem = animation.GetComponent<ParticleSystem>();
                var animationCheck = CaptureAnimationFrames(
                    camera,
                    animationSystem,
                    "fixture25-frames.png");
                var animationStats = ParticleStats(animationSystem);
                var animationCount = animationSystem.particleCount;
                checks.Add(animationCheck.value);
                captures.Add(animationCheck.file);
                animation.SetActive(false);

                var sixFace = CreateParticleSystem("fixture26 six faces", root.transform, material, cube);
                var sixFaceSystem = sixFace.GetComponent<ParticleSystem>();
                var sixFaceCheck = CaptureSixFaces(
                    camera,
                    sixFaceSystem,
                    "fixture26");
                var sixFaceStats = ParticleStats(sixFaceSystem);
                var sixFaceCount = sixFaceSystem.particleCount;
                checks.AddRange(sixFaceCheck.value);
                captures.AddRange(sixFaceCheck.files);
                sixFace.SetActive(false);

                var styles = CreateParticleSystem("catalog style visibility", root.transform, material, cube);
                var styleCheck = CaptureStyleVisibility(camera, styles.GetComponent<ParticleSystem>(), "catalog-styles.png");
                var styleStats = ParticleStats(styles.GetComponent<ParticleSystem>());
                var styleCount = styles.GetComponent<ParticleSystem>().particleCount;
                checks.Add(styleCheck.value);
                captures.Add(styleCheck.file);
                styles.SetActive(false);

                var scaleRoot = new GameObject("ContentScale fixture");
                scaleRoot.transform.SetParent(root.transform, false);
                scaleRoot.layer = 31;
                var scaleSystem = CreateParticleSystem(
                        "scale UV",
                        scaleRoot.transform,
                        material,
                        cube,
                        ParticleSystemSimulationSpace.Local)
                    .GetComponent<ParticleSystem>();
                var scaleCheck = CaptureScaleUv(camera, scaleSystem, scaleRoot.transform);
                var scaleStats = ParticleStats(scaleSystem);
                checks.AddRange(scaleCheck.value);
                captures.AddRange(scaleCheck.files);
                scaleSystem.gameObject.SetActive(false);
                var rigidTextureCheck = CaptureRigidTexture(
                    root.transform,
                    material,
                    cube);
                checks.AddRange(rigidTextureCheck.value);
                captures.AddRange(rigidTextureCheck.files);

                var stats = new
                {
                    catalogMaterial = material.name,
                    animationParticles = animationCount,
                    sixFaceParticles = sixFaceCount,
                    styleParticles = styleCount,
                    scaleParticles = scaleSystem.particleCount,
                    fixture25FrameTime = .05f,
                    fixture25Frames = 3,
                    fixture26FaceOrder = new[] { "+X", "-X", "+Y", "-Y", "+Z", "-Z" },
                    particleStats = new
                    {
                        animation = animationStats,
                        sixFace = sixFaceStats,
                        styles = styleStats,
                        scale = scaleStats
                    },
                    scope = "Actual native ParticleSystem + ParticleSystemRenderer + ParticleBoxCatalog shader; temporary Play rig"
                };
                var report = new
                {
                    success = checks.All(item => ReadBool(item, "pass")),
                    completed = true,
                    checks,
                    captures,
                    stats,
                    pixelScope = "PNG evidence is generated by Camera.Render + Texture2D.ReadPixels; this is not a VRChat client capture."
                };
                WriteJson(VisualReportPath, report);
                return JsonConvert.SerializeObject(report, Formatting.Indented);
            }
            finally
            {
                camera.targetTexture = null;
                RenderSettings.ambientLight = oldAmbient;
                RenderSettings.fog = oldFog;
                Time.timeScale = oldTimeScale;
                Object.DestroyImmediate(root);
                Object.DestroyImmediate(cube);
            }
        }

        static CheckBox CaptureAnimationFrames(Camera camera, ParticleSystem system, string file)
        {
            var particles = new ParticleSystem.Particle[3];
            var positions = new[] { new Vector3(-1f, 0f, 0f), Vector3.zero, new Vector3(1f, 0f, 0f) };
            var desired = new[] { 0, 1, 2 };
            var labels = new[] { "frame0", "frame1", "frame2" };
            var expected = new[] { "blue", "red", "purple" };
            var samples = new List<object>();
            Color32[] pixels = null;
            var basePhase = 0;
            for (var attempt = 0; attempt < 5; attempt++)
            {
                basePhase = Mathf.FloorToInt(Time.timeSinceLevelLoad / .05f) % 3;
                for (var i = 0; i < particles.Length; i++)
                {
                    particles[i] = Particle(
                        positions[i],
                        new Vector3(.55f, .55f, .55f),
                        FixtureAnimation,
                        0,
                        PhaseCode((desired[i] - basePhase + 3) % 3));
                }
                CommitParticles(system, particles, particles.Length);
                camera.orthographicSize = .9f;
                camera.transform.position = new Vector3(0f, 0f, 4f);
                camera.transform.LookAt(Vector3.zero, Vector3.up);
                pixels = Render(camera, 640, 480);
                samples.Clear();
                var all = true;
                for (var i = 0; i < particles.Length; i++)
                {
                    var point = PixelPoint(camera, positions[i], 640, 480);
                    var color = Average(pixels, 640, 480, Mathf.RoundToInt(point.x), Mathf.RoundToInt(point.y), 8);
                    var kind = ChromaticClass(color);
                    all &= kind == expected[i];
                    samples.Add(new
                    {
                        label = labels[i],
                        expected = expected[i],
                        actual = kind,
                        rgb = new[] { color.r, color.g, color.b },
                        desiredFrame = desired[i],
                        attempt,
                        basePhase
                    });
                }
                SavePng(file, pixels, 640, 480);
                if (all)
                {
                    return new CheckBox(
                        new
                        {
                            name = "fixture25 explicit animation frames blue/red/purple",
                            pass = true,
                            samples,
                            file = VisualEvidenceDirectory + "/" + Path.GetFileName(file),
                            note = "Particle A phases align one cube to each declared frame at capture."
                        },
                        VisualEvidenceDirectory + "/" + Path.GetFileName(file));
                }
            }
            return new CheckBox(
                new
                {
                    name = "fixture25 explicit animation frames blue/red/purple",
                    pass = false,
                    samples,
                    file = VisualEvidenceDirectory + "/" + Path.GetFileName(file),
                    note = "Retried five times; PNG was kept for inspection."
                },
                VisualEvidenceDirectory + "/" + Path.GetFileName(file));
        }

        static MultiCheckBox CaptureSixFaces(Camera camera, ParticleSystem system, string prefix)
        {
            CommitParticles(
                system,
                new[] { Particle(Vector3.zero, new Vector3(.55f, .55f, .55f), FixtureSixFace, 0, 0) },
                1);
            var views = new[]
            {
                new { name = "px", dir = Vector3.right, expected = "red" },
                new { name = "nx", dir = Vector3.left, expected = "green" },
                new { name = "py", dir = Vector3.up, expected = "blue" },
                new { name = "ny", dir = Vector3.down, expected = "yellow" },
                new { name = "pz", dir = Vector3.forward, expected = "cyan" },
                new { name = "nz", dir = Vector3.back, expected = "magenta" }
            };
            var checks = new List<object>();
            var files = new List<string>();
            foreach (var view in views)
            {
                camera.orthographicSize = .75f;
                camera.transform.position = view.dir * 4f;
                camera.transform.LookAt(Vector3.zero, Mathf.Abs(view.dir.y) > .9f ? Vector3.forward : Vector3.up);
                var pixels = Render(camera, 384, 384);
                var point = PixelPoint(camera, Vector3.zero, 384, 384);
                var color = Average(pixels, 384, 384, Mathf.RoundToInt(point.x), Mathf.RoundToInt(point.y), 10);
                var actual = NearestSixFace(color);
                var file = prefix + "-" + view.name + ".png";
                SavePng(file, pixels, 384, 384);
                files.Add(VisualEvidenceDirectory + "/" + Path.GetFileName(file));
                checks.Add(new
                {
                    name = "fixture26 face " + view.name,
                    pass = actual == view.expected,
                    expected = view.expected,
                    actual,
                    rgb = new[] { color.r, color.g, color.b },
                    file = VisualEvidenceDirectory + "/" + Path.GetFileName(file)
                });
            }
            return new MultiCheckBox(checks, files);
        }

        static CheckBox CaptureStyleVisibility(Camera camera, ParticleSystem system, string file)
        {
            var styles = new[] { 10, 16, 19, 21 };
            var labels = new[] { "metal", "water", "cloud", "cutout" };
            var positions = new[] { new Vector3(-1.5f, 0f, 0f), new Vector3(-.5f, 0f, 0f), new Vector3(.5f, 0f, 0f), new Vector3(1.5f, 0f, 0f) };
            var particles = styles.Select((style, index) => Particle(positions[index], new Vector3(.72f, .72f, .72f), style, 0, 0)).ToArray();
            CommitParticles(system, particles, particles.Length);
            camera.orthographicSize = 1.15f;
            camera.transform.position = new Vector3(0f, 0f, 6f);
            camera.transform.LookAt(Vector3.zero, Vector3.up);
            var pixels = Render(camera, 800, 480);
            SavePng(file, pixels, 800, 480);
            var samples = new List<object>();
            var pass = true;
            for (var i = 0; i < styles.Length; i++)
            {
                var point = PixelPoint(camera, positions[i], 800, 480);
                var foreground = ForegroundFraction(pixels, 800, 480, Mathf.RoundToInt(point.x), Mathf.RoundToInt(point.y), 35);
                var visible = foreground > .02f;
                pass &= visible;
                samples.Add(new { style = styles[i], material = labels[i], foreground, pass = visible });
            }
            return new CheckBox(
                new { name = "metal water cloud cutout visible", pass, samples, file = VisualEvidenceDirectory + "/" + Path.GetFileName(file) },
                VisualEvidenceDirectory + "/" + Path.GetFileName(file));
        }

        static MultiCheckBox CaptureScaleUv(Camera camera, ParticleSystem system, Transform scaleRoot)
        {
            CommitParticles(system, new[] { Particle(Vector3.zero, Vector3.one, 1, 0, 0) }, 1);
            camera.orthographicSize = 1.1f;
            camera.transform.position = new Vector3(0f, 0f, 4f);
            camera.transform.LookAt(Vector3.zero, Vector3.up);
            var one = Render(camera, 512, 512);
            SavePng("scale-1.png", one, 512, 512);
            scaleRoot.localScale = Vector3.one * .5f;
            camera.orthographicSize = .55f;
            camera.transform.position = new Vector3(0f, 0f, 2f);
            camera.transform.LookAt(Vector3.zero, Vector3.up);
            var half = Render(camera, 512, 512);
            SavePng("scale-half.png", half, 512, 512);
            var mae = MeanAbsoluteError(one, half);
            return new MultiCheckBox(
                new List<object>
                {
                    new
                    {
                        name = "scale 0.5 keeps local UV pattern",
                        pass = mae < .06,
                        meanAbsoluteError = mae,
                        threshold = .06,
                        files = new[] { VisualEvidenceDirectory + "/scale-1.png", VisualEvidenceDirectory + "/scale-half.png" }
                    }
                },
                new List<string> { VisualEvidenceDirectory + "/scale-1.png", VisualEvidenceDirectory + "/scale-half.png" });
        }

        static MultiCheckBox CaptureRigidTexture(
            Transform parent,
            Material material,
            Mesh cube)
        {
            var rigidRoot = new GameObject("Rigid root texture fixture");
            rigidRoot.layer = 31;
            rigidRoot.transform.SetParent(parent, false);
            var cameraObject = new GameObject("Rigid fixture camera", typeof(Camera));
            var lightObject = new GameObject("Rigid fixture light", typeof(Light));
            cameraObject.transform.SetParent(rigidRoot.transform, false);
            lightObject.transform.SetParent(rigidRoot.transform, false);
            var camera = cameraObject.GetComponent<Camera>();
            var light = lightObject.GetComponent<Light>();
            camera.orthographic = true;
            camera.orthographicSize = .75f;
            camera.nearClipPlane = .01f;
            camera.farClipPlane = 50f;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = Color.black;
            camera.cullingMask = 1 << 31;
            camera.allowMSAA = false;
            camera.allowHDR = false;
            camera.transform.localPosition = new Vector3(0, 0, 4);
            camera.transform.localRotation = Quaternion.Euler(0, 180, 0);
            light.type = LightType.Directional;
            light.intensity = 1.25f;
            light.color = Color.white;
            lightObject.transform.localRotation = Quaternion.Euler(35f, -35f, 0f);

            var particleRoot = new GameObject("Rigid fixture static six-face");
            particleRoot.layer = 31;
            particleRoot.transform.SetParent(rigidRoot.transform, false);
            var system = particleRoot.AddComponent<ParticleSystem>();
            var main = system.main;
            main.playOnAwake = false;
            main.loop = false;
            main.duration = .1f;
            main.startLifetime = 100000f;
            main.startSpeed = 0f;
            main.startSize3D = true;
            main.maxParticles = 1;
            main.simulationSpace = ParticleSystemSimulationSpace.Local;
            main.scalingMode = ParticleSystemScalingMode.Hierarchy;
            var emission = system.emission;
            emission.enabled = false;
            var shape = system.shape;
            shape.enabled = false;
            var renderer = particleRoot.GetComponent<ParticleSystemRenderer>();
            renderer.enabled = true;
            renderer.renderMode = ParticleSystemRenderMode.Mesh;
            renderer.mesh = cube;
            renderer.sharedMaterial = material;
            renderer.alignment = ParticleSystemRenderSpace.Local;
            renderer.enableGPUInstancing = false;
            renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            renderer.SetActiveVertexStreams(RequiredStreams
                .Select(stream => (ParticleSystemVertexStream)Enum.Parse(typeof(ParticleSystemVertexStream), stream))
                .ToList());
            var serialized = new SerializedObject(renderer);
            var colorSpace = serialized.FindProperty("m_ApplyActiveColorSpace");
            if (colorSpace != null)
            {
                colorSpace.boolValue = false;
                serialized.ApplyModifiedPropertiesWithoutUndo();
            }
            system.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            system.Play(false);
            CommitParticles(
                system,
                new[] { Particle(Vector3.zero, new Vector3(.55f, .55f, .55f), FixtureSixFace, 0, 0) },
                1);
            var before = Render(camera, 512, 512);
            SavePng("rigid-root-before.png", before, 512, 512);
            rigidRoot.transform.SetPositionAndRotation(
                new Vector3(3f, 1.5f, -2f),
                Quaternion.Euler(17f, 37f, -9f));
            var after = Render(camera, 512, 512);
            SavePng("rigid-root-after.png", after, 512, 512);
            var difference = CroppedPixelDifference(
                before,
                after,
                512,
                512,
                256,
                256,
                180);
            var files = new List<string>
            {
                VisualEvidenceDirectory + "/rigid-root-before.png",
                VisualEvidenceDirectory + "/rigid-root-after.png"
            };
            var result = new MultiCheckBox(
                new List<object>
                {
                    new
                    {
                        name = "static texture remains fixed after rigid root translation and rotation",
                        pass = difference.meanAbsoluteError < .01f &&
                               difference.changedFraction < .03f,
                        meanAbsoluteError = difference.meanAbsoluteError,
                        changedFraction = difference.changedFraction,
                        transform = new
                        {
                            position = new[] { 3f, 1.5f, -2f },
                            euler = new[] { 17f, 37f, -9f }
                        },
                        files
                    }
                },
                files);
            Object.DestroyImmediate(rigidRoot);
            return result;
        }

        static ParticleSystem.Particle Particle(Vector3 position, Vector3 size, int style, int tint, int motionCode)
        {
            return new ParticleSystem.Particle
            {
                position = position,
                startSize3D = size,
                startColor = new Color32(
                    CodeByte(style & 63),
                    CodeByte((style >> 6) & 63),
                    CodeByte(tint & 63),
                    CodeByte(motionCode & 63)),
                startLifetime = 100000f,
                remainingLifetime = 100000f,
                randomSeed = (uint)(style * 100 + motionCode + 1)
            };
        }

        static byte CodeByte(int code)
        {
            return (byte)Mathf.RoundToInt(Mathf.Clamp(code, 0, 63) * 255f / 63f);
        }

        static void CommitParticles(ParticleSystem system, ParticleSystem.Particle[] particles, int count)
        {
            system.SetParticles(particles, count);
            system.Simulate(0f, true, false, false);
        }

        static int PhaseCode(int offset)
        {
            switch (offset & 3)
            {
                case 0: return 0;
                case 1: return 22; // ceil(64/3): shader floor crosses exactly one frame
                case 2: return 43; // ceil(128/3): shader floor crosses exactly two frames
                default: return 0;
            }
        }

        static GameObject CreateParticleSystem(
            string name,
            Transform parent,
            Material material,
            Mesh cube,
            ParticleSystemSimulationSpace simulationSpace = ParticleSystemSimulationSpace.World)
        {
            var go = new GameObject(name);
            go.layer = 31;
            go.transform.SetParent(parent, false);
            var system = go.AddComponent<ParticleSystem>();
            var main = system.main;
            main.playOnAwake = false;
            main.loop = false;
            main.duration = .1f;
            main.startLifetime = 100000f;
            main.startSpeed = 0f;
            main.startSize3D = true;
            main.maxParticles = 16;
            main.simulationSpace = simulationSpace;
            main.scalingMode = ParticleSystemScalingMode.Hierarchy;
            var emission = system.emission;
            emission.enabled = false;
            var shape = system.shape;
            shape.enabled = false;
            var renderer = go.GetComponent<ParticleSystemRenderer>();
            if (!renderer)
                throw new InvalidOperationException("ParticleSystemRenderer was not created for fixture " + name);
            renderer.enabled = true;
            renderer.renderMode = ParticleSystemRenderMode.Mesh;
            renderer.mesh = cube;
            renderer.sharedMaterial = material;
            renderer.alignment = ParticleSystemRenderSpace.Local;
            renderer.enableGPUInstancing = false;
            renderer.shadowCastingMode = UnityEngine.Rendering.ShadowCastingMode.Off;
            renderer.receiveShadows = false;
            var active = RequiredStreams
                .Select(stream => (ParticleSystemVertexStream)Enum.Parse(typeof(ParticleSystemVertexStream), stream))
                .ToList();
            renderer.SetActiveVertexStreams(active);
            var serialized = new SerializedObject(renderer);
            var colorSpace = serialized.FindProperty("m_ApplyActiveColorSpace");
            if (colorSpace != null)
            {
                colorSpace.boolValue = false;
                serialized.ApplyModifiedPropertiesWithoutUndo();
            }
            system.Stop(true, ParticleSystemStopBehavior.StopEmittingAndClear);
            system.Play(false);
            return go;
        }

        static Mesh CreateCubeMesh()
        {
            var primitive = GameObject.CreatePrimitive(PrimitiveType.Cube);
            var mesh = UnityEngine.Object.Instantiate(primitive.GetComponent<MeshFilter>().sharedMesh);
            mesh.name = "MorphForge material fixture cube";
            // Mirror ParticleStage2Build's face-local UV contract:
            // uv = tileUV, uv2 = (axis, 0). The catalog shader must never
            // receive a standard primitive cube UV that lacks the axis tag.
            var vertices = mesh.vertices;
            var normals = mesh.normals;
            var tileUV = new Vector2[vertices.Length];
            var faceUV = new Vector2[vertices.Length];
            for (var i = 0; i < vertices.Length; i++)
            {
                var vertex = vertices[i];
                var normal = normals[i];
                var axis = Mathf.Abs(normal.x) > .5f ? 0 : (Mathf.Abs(normal.y) > .5f ? 1 : 2);
                tileUV[i] = axis == 0
                    ? new Vector2(vertex.z + .5f, vertex.y + .5f)
                    : axis == 1
                        ? new Vector2(vertex.x + .5f, vertex.z + .5f)
                        : new Vector2(vertex.x + .5f, vertex.y + .5f);
                faceUV[i] = new Vector2(axis, 0f);
            }
            mesh.uv = tileUV;
            mesh.uv2 = faceUV;
            Object.DestroyImmediate(primitive);
            return mesh;
        }

        static Color32[] Render(Camera camera, int width, int height)
        {
            var previousTarget = camera.targetTexture;
            var previousActive = RenderTexture.active;
            RenderTexture target = null;
            Texture2D texture = null;
            try
            {
                target = RenderTexture.GetTemporary(width, height, 24, RenderTextureFormat.ARGB32);
                camera.targetTexture = target;
                camera.aspect = (float)width / height;
                camera.Render();
                RenderTexture.active = target;
                texture = new Texture2D(width, height, TextureFormat.RGB24, false);
                texture.ReadPixels(new Rect(0, 0, width, height), 0, 0);
                texture.Apply();
                return texture.GetPixels32();
            }
            finally
            {
                camera.targetTexture = previousTarget;
                RenderTexture.active = previousActive;
                if (target) RenderTexture.ReleaseTemporary(target);
                if (texture) Object.DestroyImmediate(texture);
            }
        }

        static object ParticleStats(ParticleSystem system)
        {
            var particles = new ParticleSystem.Particle[system.main.maxParticles];
            var count = system.GetParticles(particles);
            var renderer = system.GetComponent<ParticleSystemRenderer>();
            var streams = new List<ParticleSystemVertexStream>();
            renderer.GetActiveVertexStreams(streams);
            return new
            {
                count,
                capacity = system.main.maxParticles,
                boundsCenter = new[] { renderer.bounds.center.x, renderer.bounds.center.y, renderer.bounds.center.z },
                boundsSize = new[] { renderer.bounds.size.x, renderer.bounds.size.y, renderer.bounds.size.z },
                material = renderer.sharedMaterial ? renderer.sharedMaterial.name : null,
                shader = renderer.sharedMaterial && renderer.sharedMaterial.shader ? renderer.sharedMaterial.shader.name : null,
                streams = streams.Select(stream => stream.ToString()).ToArray()
            };
        }

        static void SavePng(string relativePath, Color32[] pixels, int width, int height)
        {
            var texture = new Texture2D(width, height, TextureFormat.RGB24, false);
            try
            {
                texture.SetPixels32(pixels);
                texture.Apply();
                var evidencePath = relativePath.StartsWith("Docs/", StringComparison.Ordinal)
                    ? relativePath
                    : VisualEvidenceDirectory + "/" + relativePath.TrimStart('/');
                var full = FindProjectPath(evidencePath);
                Directory.CreateDirectory(Path.GetDirectoryName(full));
                File.WriteAllBytes(full, texture.EncodeToPNG());
            }
            finally
            {
                Object.DestroyImmediate(texture);
            }
        }

        static Color Average(Color32[] pixels, int width, int height, int x, int y, int radius)
        {
            long red = 0, green = 0, blue = 0, count = 0;
            for (var py = Mathf.Max(0, y - radius); py <= Mathf.Min(height - 1, y + radius); py++)
            for (var px = Mathf.Max(0, x - radius); px <= Mathf.Min(width - 1, x + radius); px++)
            {
                var color = pixels[py * width + px];
                red += color.r;
                green += color.g;
                blue += color.b;
                count++;
            }
            return count == 0 ? Color.black : new Color(red / (255f * count), green / (255f * count), blue / (255f * count));
        }

        static Vector2 PixelPoint(Camera camera, Vector3 world, int width, int height)
        {
            var viewport = camera.WorldToViewportPoint(world);
            return new Vector2(viewport.x * width, viewport.y * height);
        }

        static float ForegroundFraction(Color32[] pixels, int width, int height, int x, int y, int radius)
        {
            var foreground = 0;
            var total = 0;
            for (var py = Mathf.Max(0, y - radius); py <= Mathf.Min(height - 1, y + radius); py++)
            for (var px = Mathf.Max(0, x - radius); px <= Mathf.Min(width - 1, x + radius); px++)
            {
                var color = pixels[py * width + px];
                total++;
                if (Mathf.Max(color.r, Mathf.Max(color.g, color.b)) > 3) foreground++;
            }
            return total == 0 ? 0f : foreground / (float)total;
        }

        static double MeanAbsoluteError(Color32[] first, Color32[] second)
        {
            if (first.Length != second.Length)
                throw new InvalidOperationException("Scale screenshot sizes differ.");
            long error = 0;
            for (var i = 0; i < first.Length; i++)
            {
                error += Math.Abs(first[i].r - second[i].r);
                error += Math.Abs(first[i].g - second[i].g);
                error += Math.Abs(first[i].b - second[i].b);
            }
            return error / (double)(first.Length * 3 * 255);
        }

        static PixelDifference CroppedPixelDifference(
            Color32[] first,
            Color32[] second,
            int width,
            int height,
            int centerX,
            int centerY,
            int radius)
        {
            long error = 0;
            var total = 0;
            var changed = 0;
            for (var y = Mathf.Max(0, centerY - radius); y <= Mathf.Min(height - 1, centerY + radius); y++)
            for (var x = Mathf.Max(0, centerX - radius); x <= Mathf.Min(width - 1, centerX + radius); x++)
            {
                var a = first[y * width + x];
                var b = second[y * width + x];
                var delta = Math.Abs(a.r - b.r) + Math.Abs(a.g - b.g) + Math.Abs(a.b - b.b);
                error += delta;
                if (delta > 18) changed++;
                total++;
            }
            return new PixelDifference
            {
                meanAbsoluteError = total == 0 ? 0f : error / (total * 3f * 255f),
                changedFraction = total == 0 ? 0f : changed / (float)total
            };
        }

        static string ChromaticClass(Color color)
        {
            if (color.r > .12f && color.b > .12f && color.g < color.r * .75f && color.g < color.b * .75f)
                return "purple";
            if (color.b > color.r * 1.35f && color.b > color.g * 1.35f)
                return "blue";
            if (color.r > color.g * 1.35f && color.r > color.b * 1.35f)
                return "red";
            return "unknown";
        }

        static string NearestSixFace(Color color)
        {
            var actual = new[] { color.r, color.g, color.b };
            var candidates = new[]
            {
                new { name = "red", rgb = new[] { 1f, 0f, 0f } },
                new { name = "green", rgb = new[] { 0f, 1f, 0f } },
                new { name = "blue", rgb = new[] { 0f, 0f, 1f } },
                new { name = "yellow", rgb = new[] { 1f, 1f, 0f } },
                new { name = "cyan", rgb = new[] { 0f, 1f, 1f } },
                new { name = "magenta", rgb = new[] { 1f, 0f, 1f } }
            };
            return candidates.OrderBy(candidate =>
                actual.Select((value, index) => Mathf.Abs(value - candidate.rgb[index])).Sum()
            ).First().name;
        }

        static bool ReadBool(object item, string name)
        {
            var value = item.GetType().GetProperty(name)?.GetValue(item);
            return value is bool flag && flag;
        }

        static object BindPrefab(string path, Material material, bool required)
        {
            var asset = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (!asset)
            {
                if (required) throw new FileNotFoundException("Required Stage4 prefab missing", path);
                return new { path, present = false, success = true, skipped = true };
            }
            var contents = PrefabUtility.LoadPrefabContents(path);
            try
            {
                var modules = FindModules(contents.transform);
                if (modules.Count == 0)
                    throw new InvalidOperationException("No " + ModuleName + " module in " + path);
                var result = modules.Select(module => BindModule(module, material, true, .5f)).ToArray();
                var saved = PrefabUtility.SaveAsPrefabAsset(contents, path);
                if (saved == null)
                    throw new InvalidOperationException("Failed to save Stage4 prefab: " + path);
                AssetDatabase.ImportAsset(path, ImportAssetOptions.ForceUpdate);
                return new { path, present = true, success = true, modules = result };
            }
            finally
            {
                PrefabUtility.UnloadPrefabContents(contents);
            }
        }

        static object BindModule(Transform module, Material material, bool recordUndo, float pumpLifetime)
        {
            var systems = module.GetComponentsInChildren<ParticleSystem>(true)
                .Where(system => system.name == "Store" || system.name == "Pump")
                .ToArray();
            var stores = systems.Where(system => system.name == "Store").ToArray();
            var pumps = systems.Where(system => system.name == "Pump").ToArray();
            if (stores.Length != 1 || pumps.Length != 1)
                throw new InvalidOperationException(
                    "Expected exactly one Store and one Pump under " + module.name +
                    "; found " + stores.Length + " Store and " + pumps.Length + " Pump.");
            if (recordUndo)
                Undo.RegisterCompleteObjectUndo(new Object[] { stores[0], stores[0].GetComponent<ParticleSystemRenderer>(), pumps[0] }, "MorphForge Stage4 finish");
            var store = stores[0];
            var renderer = store.GetComponent<ParticleSystemRenderer>();
            if (!renderer)
                throw new InvalidOperationException("Store has no ParticleSystemRenderer in " + module.name);
            renderer.sharedMaterial = material;
            renderer.renderMode = ParticleSystemRenderMode.Mesh;
            var active = RequiredStreams
                .Select(stream => (ParticleSystemVertexStream)Enum.Parse(typeof(ParticleSystemVertexStream), stream))
                .ToList();
            renderer.SetActiveVertexStreams(active);
            var serialized = new SerializedObject(renderer);
            var activeColorSpace = serialized.FindProperty("m_ApplyActiveColorSpace");
            if (activeColorSpace != null)
            {
                activeColorSpace.boolValue = false;
                serialized.ApplyModifiedPropertiesWithoutUndo();
            }
            var pump = pumps[0].main;
            pump.startLifetime = pumpLifetime;
            EditorUtility.SetDirty(store);
            EditorUtility.SetDirty(renderer);
            EditorUtility.SetDirty(pumps[0]);
            if (PrefabUtility.IsPartOfPrefabInstance(renderer))
                PrefabUtility.RecordPrefabInstancePropertyModifications(renderer);
            if (PrefabUtility.IsPartOfPrefabInstance(pumps[0]))
                PrefabUtility.RecordPrefabInstancePropertyModifications(pumps[0]);
            return new
            {
                module = module.name,
                store = store.name,
                pump = pumps[0].name,
                material = material ? material.name : null,
                shader = material && material.shader ? material.shader.name : null,
                pumpLifetime = pump.startLifetime.constant,
                streams = active.Select(value => value.ToString()).ToArray(),
                normal = active.Contains(ParticleSystemVertexStream.Normal),
                uv = active.Contains(ParticleSystemVertexStream.UV),
                uv2 = active.Contains(ParticleSystemVertexStream.UV2),
                sizeXyz = active.Contains(ParticleSystemVertexStream.SizeXYZ)
            };
        }

        static List<Transform> FindModules(Transform root)
        {
            var result = new List<Transform>();
            if (root.name == ModuleName) result.Add(root);
            result.AddRange(root.GetComponentsInChildren<Transform>(true)
                .Where(transform => transform != root && transform.name == ModuleName));
            if (result.Count == 0)
            {
                // The reusable Geometry Display prefab is a standalone display
                // root. Placement instantiates and renames the instance to
                // MorphForgeGeometry. Accept that standalone root only when it
                // has exactly one Store and one Pump; never guess in an Avatar.
                var systems = root.GetComponentsInChildren<ParticleSystem>(true);
                if (systems.Count(system => system.name == "Store") == 1 &&
                    systems.Count(system => system.name == "Pump") == 1)
                    result.Add(root);
            }
            return result;
        }

        static object ApplyToActiveScene(Material material, List<SceneSnapshot> beforeScenes)
        {
            var scene = SceneManager.GetActiveScene();
            var roots = scene.GetRootGameObjects();
            var avatars = roots.SelectMany(root => root.GetComponentsInChildren<VRC.SDK3.Avatars.Components.VRCAvatarDescriptor>(true)).ToArray();
            var wasDirty = beforeScenes
                .Where(item => item.path == scene.path)
                .Select(item => (bool?)item.isDirty)
                .FirstOrDefault() ?? scene.isDirty;
            if (string.IsNullOrEmpty(scene.path))
                return new { path = scene.path, modified = false, saved = false, reason = "Active scene is untitled; not saved." };
            if (wasDirty)
                return new { path = scene.path, modified = false, saved = false, reason = "Scene was dirty before Finish; preserved untouched." };
            if (avatars.Length != 1)
                return new { path = scene.path, modified = false, saved = false, reason = "Scene does not have exactly one Avatar." };
            var modules = FindModules(avatars[0].transform);
            if (modules.Count != 1)
                return new { path = scene.path, modified = false, saved = false, reason = "Scene does not have exactly one MorphForgeGeometry module." };

            var poses = CaptureSceneTransforms(scene);
            var result = BindModule(modules[0], material, true, .5f);
            foreach (var pose in poses)
            {
                if (!pose.transform) continue;
                if ((pose.transform.position - pose.position).sqrMagnitude > 1e-10f ||
                    Quaternion.Angle(pose.transform.rotation, pose.rotation) > .001f ||
                    (pose.transform.localScale - pose.scale).sqrMagnitude > 1e-10f)
                    throw new InvalidOperationException("Finish changed a protected scene transform: " + pose.path);
            }
            EditorSceneManager.MarkSceneDirty(scene);
            if (!EditorSceneManager.SaveScene(scene))
                throw new InvalidOperationException("Failed to save clean single-Avatar scene.");
            return new
            {
                path = scene.path,
                modified = true,
                saved = true,
                avatar = avatars[0].name,
                module = modules[0].name,
                binding = result,
                preservedTransforms = poses.Count
            };
        }

        static List<PoseSnapshot> CaptureSceneTransforms(Scene scene)
        {
            return scene.GetRootGameObjects()
                .SelectMany(root => root.GetComponentsInChildren<Transform>(true))
                .Select(transform => new PoseSnapshot
                {
                    path = transform == null ? "missing" : transform.name,
                    transform = transform,
                    position = transform.position,
                    rotation = transform.rotation,
                    scale = transform.localScale
                })
                .ToList();
        }

        static object BuildFixtureScene()
        {
            var current = SceneManager.GetActiveScene();
            var path = ValidationScenePath;
            if (AssetDatabase.LoadAssetAtPath<SceneAsset>(path) != null)
                path = AssetDatabase.GenerateUniqueAssetPath(
                    "Assets/MorphForge/Scenes/MorphForge_MaterialsValidation_" +
                    DateTime.UtcNow.ToString("yyyyMMdd_HHmmss") + ".unity");
            var fixture = EditorSceneManager.NewScene(NewSceneSetup.EmptyScene, NewSceneMode.Additive);
            SceneManager.SetActiveScene(fixture);
            var root = new GameObject("Material Catalog Validation Fixture");
            root.tag = "EditorOnly";
            var cameraObject = new GameObject("Fixture Camera", typeof(Camera));
            cameraObject.tag = "MainCamera";
            var camera = cameraObject.GetComponent<Camera>();
            camera.orthographic = true;
            camera.orthographicSize = 1.2f;
            camera.clearFlags = CameraClearFlags.SolidColor;
            camera.backgroundColor = Color.black;
            camera.transform.position = new Vector3(2.5f, 2f, 4f);
            camera.transform.LookAt(Vector3.zero, Vector3.up);
            var lightObject = new GameObject("Fixture Directional Light", typeof(Light));
            var light = lightObject.GetComponent<Light>();
            light.type = LightType.Directional;
            light.color = Color.white;
            light.intensity = 1.25f;
            lightObject.transform.rotation = Quaternion.Euler(35f, -35f, 0f);
            EditorSceneManager.MarkSceneDirty(fixture);
            if (!EditorSceneManager.SaveScene(fixture, path))
                throw new InvalidOperationException("Failed to save material validation fixture scene.");
            EditorSceneManager.CloseScene(fixture, true);
            if (current.IsValid())
                SceneManager.SetActiveScene(current);
            return new
            {
                path,
                created = true,
                note = "Static Edit Mode rig only; ValidateVisual creates and removes the native ParticleSystem rig."
            };
        }

        static List<SceneSnapshot> SnapshotScenes()
        {
            var result = new List<SceneSnapshot>();
            for (var i = 0; i < SceneManager.sceneCount; i++)
            {
                var scene = SceneManager.GetSceneAt(i);
                result.Add(new SceneSnapshot
                {
                    path = scene.path,
                    isDirty = scene.isDirty
                });
            }
            return result;
        }

        static CatalogView ReadCatalog()
        {
            var path = FindProjectPath(MaterialCatalogPath);
            if (!File.Exists(path)) throw new FileNotFoundException("Missing material catalog", path);
            var document = JsonConvert.DeserializeObject<CatalogMaterialBuild.CatalogDocument>(File.ReadAllText(path));
            if (document == null || document.format != CatalogMaterialBuild.Format || document.materials == null)
                throw new InvalidDataException("Material catalog has an invalid format.");
            if (document.materials.Count < 2)
                throw new InvalidDataException("Material catalog is unexpectedly empty.");
            return new CatalogView { materials = document.materials };
        }

        static FixtureView FindFixture(CatalogView catalog)
        {
            var animation = catalog.materials.FirstOrDefault(material => material.id == FixtureAnimation);
            var sixFace = catalog.materials.FirstOrDefault(material => material.id == FixtureSixFace);
            if (animation == null || sixFace == null)
                throw new InvalidDataException("Material catalog must contain style 25 and style 26 fixtures.");
            if (animation.frames != 3 || Mathf.Abs(animation.frameTime - .05f) > .001f)
                throw new InvalidDataException("Style 25 must contain three frames at 0.05 seconds per frame.");
            if (sixFace.textures == null || sixFace.textures.Count != 6)
                throw new InvalidDataException("Style 26 must contain exactly six face textures.");
            return new FixtureView
            {
                animation = new
                {
                    id = animation.id,
                    uid = animation.uid,
                    frames = animation.frames,
                    frameTime = animation.frameTime,
                    expected = new[] { "blue", "red", "purple" }
                },
                sixFace = new
                {
                    id = sixFace.id,
                    uid = sixFace.uid,
                    textures = sixFace.textures.Count,
                    expected = new[] { "+X red", "-X green", "+Y blue", "-Y yellow", "+Z cyan", "-Z magenta" }
                }
            };
        }

        static PrefabInspection InspectPrefab(string path, bool required)
        {
            var asset = AssetDatabase.LoadAssetAtPath<GameObject>(path);
            if (!asset)
                return new PrefabInspection { path = path, required = required, present = false, success = !required };
            var contents = PrefabUtility.LoadPrefabContents(path);
            try
            {
                var modules = FindModules(contents.transform);
                if (modules.Count == 0)
                    return new PrefabInspection { path = path, required = required, present = true, success = false, reason = "No module" };
                var systems = modules.SelectMany(module => module.GetComponentsInChildren<ParticleSystem>(true))
                    .Where(system => system.name == "Store" || system.name == "Pump").ToArray();
                var stores = systems.Where(system => system.name == "Store").ToArray();
                var pumps = systems.Where(system => system.name == "Pump").ToArray();
                var material = stores.Select(store => store.GetComponent<ParticleSystemRenderer>()).Where(renderer => renderer).Select(renderer => renderer.sharedMaterial).FirstOrDefault();
                var renderer = stores.Select(store => store.GetComponent<ParticleSystemRenderer>()).FirstOrDefault(item => item);
                var streams = new List<ParticleSystemVertexStream>();
                if (renderer) renderer.GetActiveVertexStreams(streams);
                var success = stores.Length > 0 && pumps.Length > 0 && material != null &&
                              material.shader && material.shader.name == "MorphForge/ParticleBoxCatalog" &&
                              pumps.All(pump => Mathf.Abs(pump.main.startLifetime.constant - .5f) < .001f) &&
                              streams.Contains(ParticleSystemVertexStream.Normal) &&
                              streams.Contains(ParticleSystemVertexStream.UV) &&
                              streams.Contains(ParticleSystemVertexStream.UV2) &&
                              streams.Contains(ParticleSystemVertexStream.SizeXYZ);
                return new PrefabInspection
                {
                    path = path,
                    required = required,
                    present = true,
                    success = success,
                    modules = modules.Count,
                    stores = stores.Length,
                    pumps = pumps.Length,
                    material = material == null ? null : material.name,
                    shader = material && material.shader ? material.shader.name : null,
                    pumpLifetimes = pumps.Select(pump => pump.main.startLifetime.constant).ToArray(),
                    streams = streams.Select(stream => stream.ToString()).ToArray()
                };
            }
            finally
            {
                PrefabUtility.UnloadPrefabContents(contents);
            }
        }

        static AnimationClip FindFireClip()
        {
            var controller = AssetDatabase.LoadAssetAtPath<UnityEditor.Animations.AnimatorController>(
                Mf3DisplayBuild.Root + "/FX.controller");
            return controller == null
                ? null
                : controller.animationClips.FirstOrDefault(clip => clip != null && clip.name == "Fire");
        }

        static void RequireIdleEditMode()
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                throw new InvalidOperationException("Stage4 finish requires idle Edit Mode.");
        }

        static string FindProjectPath(string relative)
        {
            return Path.GetFullPath(Path.Combine(Application.dataPath, "..", relative));
        }

        static void WriteJson(string relative, object value)
        {
            var path = FindProjectPath(relative);
            Directory.CreateDirectory(Path.GetDirectoryName(path));
            File.WriteAllText(path, JsonConvert.SerializeObject(value, Formatting.Indented) + Environment.NewLine);
        }

        sealed class CatalogView
        {
            public List<CatalogMaterialBuild.CatalogMaterial> materials;
        }

        sealed class FixtureView
        {
            public object animation;
            public object sixFace;
        }

        sealed class SceneSnapshot
        {
            public string path;
            public bool isDirty;
        }

        sealed class PoseSnapshot
        {
            public string path;
            public Transform transform;
            public Vector3 position;
            public Quaternion rotation;
            public Vector3 scale;
        }

        sealed class PrefabInspection
        {
            public string path;
            public bool required;
            public bool present;
            public bool success;
            public string reason;
            public int modules;
            public int stores;
            public int pumps;
            public string material;
            public string shader;
            public float[] pumpLifetimes;
            public string[] streams;
        }

        sealed class CheckBox
        {
            public CheckBox(object value, string file)
            {
                this.value = value;
                this.file = file;
            }

            public object value;
            public string file;
        }

        sealed class MultiCheckBox
        {
            public MultiCheckBox(List<object> value, List<string> files)
            {
                this.value = value;
                this.files = files;
            }

            public List<object> value;
            public List<string> files;
        }

        sealed class PixelDifference
        {
            public float meanAbsoluteError;
            public float changedFraction;
        }
    }
}
