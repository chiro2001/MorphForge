using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using Newtonsoft.Json;
using UnityEditor;
using UnityEngine;

namespace MorphForge.Editor
{
    /// <summary>
    /// Builds the single-renderer material catalog from the external VRC
    /// material manifest. This is an Editor-only consumer: it never runs on
    /// the Avatar and never creates an Animator or a runtime MonoBehaviour.
    /// </summary>
    public static class CatalogMaterialBuild
    {
        public const string Format = "MorphForgeMaterialCatalog/1";
        public const string DefaultSourceSubdirectory = "Docs/Materials";
        public const string DefaultOutputRoot = "Assets/MorphForge/MaterialsCatalog";
        public static string OutputRoot { get; private set; } = DefaultOutputRoot;
        public static string AtlasAssetPath => OutputRoot + "/CatalogAtlas.png";
        public static string MetadataAssetPath => OutputRoot + "/CatalogMetadata.asset";
        public static string MaterialAssetPath => OutputRoot + "/ParticleBoxCatalog.mat";

        public const int MaxStyleId = 4095;
        public const int MaxFrames = 1024;
        public static int CellPixels {get; private set;} = 32;
        static bool LayeredOnly;
        static int LayerBankSize;
        static Texture2DArray[] LayerBanks=new Texture2DArray[4];
        static Texture2D FaceMap;
        static int UniqueSlices;
        // Keep the 4096-style address space in a compact two-dimensional data texture.
        // The former 16384x1 layout failed VRChat server checks; 256x64 passed.
        public const int MetadataWidth = 256;
        public const int MetadataHeight = 64;
        public const int MetadataTexelCount = (MaxStyleId + 1) * 4;

        public static string LayersAssetPath => OutputRoot + "/CatalogLayers.asset";
        const int HasTextureFlag = 1;
        const int TransparentFlag = 2;
        const int CutoutFlag = 4;
        const int WaterFlag = 8;
        const int CloudFlag = 16;

        [MenuItem("Tools/MorphForge/Materials/Validate catalog")]
        static void MenuValidate()
        {
            Debug.Log(Validate());
        }

        [MenuItem("Tools/MorphForge/Materials/Build catalog material")]
        static void MenuBuild()
        {
            Debug.Log(Build());
        }

        public static string BuildAt(string sourceRoot, string outputRoot)
       {
           if (!outputRoot.StartsWith("Assets/MorphForge/Diagnostics/", StringComparison.Ordinal) || outputRoot.Contains(".."))
               throw new InvalidOperationException("Isolated output must be under Assets/MorphForge/Diagnostics/.");
           var previous = OutputRoot;
           try { OutputRoot = outputRoot; return Build(sourceRoot); }
           finally { OutputRoot = previous; }
       }

        public static string Build()
        {
            return Build(DefaultSourceRoot);
        }

        public static string Build(string sourceRoot)
        {
            if (EditorApplication.isPlaying || EditorApplication.isCompiling)
                throw new InvalidOperationException("Material catalog build requires idle Edit Mode.");

            var root = ResolveSourceRoot(sourceRoot);
            var options=Newtonsoft.Json.Linq.JObject.Parse(File.ReadAllText(Path.Combine(root,"catalog.json")))["textureOptions"];
            CellPixels=(int?)options?["cellPixels"]??32;
            LayeredOnly=(bool?)options?["layeredOnly"]??false;
            if(CellPixels!=16&&CellPixels!=32&&CellPixels!=64)throw new InvalidDataException("cellPixels must be 16, 32 or 64");

            var materials = LoadManifest(root);
            var prepared = Prepare(materials, root);
            var layout = Pack(prepared);
            if (MetadataWidth * MetadataHeight != MetadataTexelCount)
                throw new InvalidOperationException("Metadata layout does not cover all style IDs.");
            if (SystemInfo.maxTextureSize < Mathf.Max(MetadataWidth, MetadataHeight))
                throw new InvalidOperationException(
                    "GPU max texture size is below the 256x64 metadata requirement.");
            EnsureFolder(OutputRoot);

            WriteAtlas(layout);
            var layers = WriteLayers(layout);
            if(LayeredOnly&&!layers&&prepared.Any(p=>p.Textures!=null))throw new InvalidOperationException("Layered-only catalog requires texture arrays");
            WriteMetadata(layout);
            var atlas = AssetDatabase.LoadAssetAtPath<Texture2D>(AtlasAssetPath);
            var metadata = AssetDatabase.LoadAssetAtPath<Texture2D>(MetadataAssetPath);
            if (!atlas || !metadata)
                throw new InvalidOperationException("Catalog texture assets were not created.");

            var shader = Shader.Find("MorphForge/ParticleBoxCatalog");
            if (!shader)
                throw new InvalidOperationException("ParticleBoxCatalog shader is not imported.");

            var generated = new Material(shader) { name = "ParticleBoxCatalog" };
            generated.SetTexture("_CatalogAtlas", atlas);
           generated.SetTexture("_CatalogLayers", layers);
           generated.SetFloat("_UseLayeredTextures", layers ? 1f : 0f);
           generated.SetFloat("_UseFaceMap", layers&&FaceMap ? 1f : 0f);
           generated.SetTexture("_CatalogFaceMap",FaceMap);
           generated.SetVector("_FaceMapSize",FaceMap?new Vector4(FaceMap.width,FaceMap.height,0,0):Vector4.one);
           generated.SetFloat("_LayerBankSize",Mathf.Max(1,LayerBankSize));
           for(int bank=1;bank<4;bank++)generated.SetTexture("_CatalogLayers"+bank,LayerBanks[bank]);
           generated.SetFloat("_VoxelGrid", 0f);
            generated.SetTexture("_CatalogMeta", metadata);
            generated.SetVector("_AtlasSize", new Vector4(
                layout.Width,
                layout.Height,
                1f / Mathf.Max(1, layout.Width),
                1f / Mathf.Max(1, layout.Height)));
            generated.SetFloat("_CellPixels", CellPixels);
            generated.SetFloat("_MetaWidth", MetadataWidth);
            generated.SetFloat("_MetaHeight", MetadataHeight);
            generated.SetFloat("_CellSize", 0.25f);

            var existing = AssetDatabase.LoadAssetAtPath<Material>(MaterialAssetPath);
            if (existing)
            {
                EditorUtility.CopySerialized(generated, existing);
                UnityEngine.Object.DestroyImmediate(generated);
                generated = existing;
                EditorUtility.SetDirty(generated);
            }
            else
            {
                AssetDatabase.CreateAsset(generated, MaterialAssetPath);
            }

            AssetDatabase.SaveAssets();
            var report = JsonConvert.SerializeObject(new
            {
                success = true,
                format = Format,
                sourceRoot = root,
                atlas = AtlasAssetPath,
                metadata = MetadataAssetPath,
                material = MaterialAssetPath,
                styleCount = prepared.Count,
                styleIdRange = new[] { prepared.Min(item => item.Material.id), prepared.Max(item => item.Material.id) },
                atlasSize = new[] { layout.Width, layout.Height },
                cellPixels = CellPixels,
                metadataWidth = MetadataWidth,
                metadataHeight = MetadataHeight,
                metadataTexels = MetadataTexelCount,
                metadataStyles = MaxStyleId + 1,
                animatedStyles = prepared.Count(item => item.Frames > 1),
                transparentStyles = prepared.Count(item => (item.Flags & TransparentFlag) != 0),
                cutoutStyles = prepared.Count(item => (item.Flags & CutoutFlag) != 0),
                layeredTextures = LayerBanks.Where(t=>t).Sum(t=>t.depth),
                uniqueTextureSlices=UniqueSlices,
                layerBanks=LayerBanks.Count(t=>t),
                atlasGpuBytes=UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(atlas),
                layerGpuBytes=LayerBanks.Where(t=>t).Sum(t=>UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(t)),
                faceMapBytes=FaceMap?UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(FaceMap):0,
                metadataBytes=UnityEngine.Profiling.Profiler.GetRuntimeMemorySizeLong(metadata),
                texturedStyles = prepared.Count(item => (item.Flags & HasTextureFlag) != 0),
                unverified = "Requires the long-run Unity gate before visual/NDMF acceptance."
            }, Formatting.Indented);
            WriteReport("unity-catalog-build.json", report);
            return report;
        }

        public static string Validate()
        {
            var root = ResolveSourceRoot(DefaultSourceRoot);
            var materials = LoadManifest(root);
            var prepared = Prepare(materials, root);
            var report = JsonConvert.SerializeObject(new
            {
                success = true,
                format = Format,
                sourceRoot = root,
                styleCount = prepared.Count,
                styleIdRange = new[] { prepared.Min(item => item.Material.id), prepared.Max(item => item.Material.id) },
                animatedStyles = prepared.Count(item => item.Frames > 1),
                transparentStyles = prepared.Count(item => (item.Flags & TransparentFlag) != 0),
                cutoutStyles = prepared.Count(item => (item.Flags & CutoutFlag) != 0),
                texturedStyles = prepared.Count(item => (item.Flags & HasTextureFlag) != 0),
                filesChecked = prepared.Where(item => item.Textures != null)
                    .SelectMany(item => item.Textures).Select(texture => texture.Path).Distinct().Count()
            }, Formatting.Indented);
            WriteReport("unity-catalog-validate.json", report);
            return report;
        }

        static string DefaultSourceRoot
        {
            get
            {
                var projectRoot = Path.GetFullPath(Path.Combine(Application.dataPath, ".."));
                var configured = Path.Combine(projectRoot, DefaultSourceSubdirectory);
                if (Directory.Exists(configured)) return configured;
                var lowercase = Path.Combine(projectRoot, "docs", "materials");
                return Directory.Exists(lowercase) ? lowercase : configured;
            }
        }

        static string ResolveSourceRoot(string sourceRoot)
        {
            var root = Path.GetFullPath(string.IsNullOrWhiteSpace(sourceRoot) ? DefaultSourceRoot : sourceRoot);
            if (!Directory.Exists(root))
                throw new DirectoryNotFoundException("Material catalog source directory not found: " + root);
            return root;
        }

        static List<CatalogMaterial> LoadManifest(string root)
        {
            var path = Path.Combine(root, "catalog.json");
            if (!File.Exists(path))
                throw new FileNotFoundException("Missing catalog.json", path);
            var document = JsonConvert.DeserializeObject<CatalogDocument>(File.ReadAllText(path));
            if (document == null || document.format != Format)
                throw new InvalidDataException("Expected " + Format + " in " + path);
            if (document.materials == null || document.materials.Count == 0)
                throw new InvalidDataException("catalog.json has no materials");

            var ids = new HashSet<int>();
            var uids = new HashSet<string>(StringComparer.Ordinal);
            foreach (var material in document.materials)
            {
                if(material != null) material.SurfaceMasks = document.capabilities != null && document.capabilities.Contains("transparent-face-mask/1");
                if (material == null)
                    throw new InvalidDataException("catalog.json contains a null material");
                if (material.id < 1 || material.id > MaxStyleId)
                    throw new InvalidDataException("styleId must be 1..4095: " + material.id);
                if (!ids.Add(material.id))
                    throw new InvalidDataException("Duplicate styleId: " + material.id);
                if (string.IsNullOrWhiteSpace(material.uid) || !uids.Add(material.uid))
                    throw new InvalidDataException("Missing or duplicate material uid for style " + material.id);
                if (string.IsNullOrWhiteSpace(material.renderMode) ||
                    (material.renderMode != "opaque" && material.renderMode != "transparent" && material.renderMode != "cutout"))
                    throw new InvalidDataException("Unsupported renderMode for style " + material.id);
                if (material.color == null || material.color.Length != 3)
                    throw new InvalidDataException("color must have three sRGB channels for style " + material.id);
                for (var i = 0; i < 3; i++)
                {
                    var channel = material.color[i];
                    if (float.IsNaN(channel) || float.IsInfinity(channel) || channel < 0f || channel > 1f)
                        throw new InvalidDataException("color is outside sRGB [0,1] for style " + material.id);
                }
                if (!Finite(material.metallic) || material.metallic < 0f || material.metallic > 1f ||
                    !Finite(material.roughness) || material.roughness < 0f || material.roughness > 1f ||
                    !Finite(material.opacity) || material.opacity < 0f || material.opacity > 1f ||
                    !Finite(material.emission) || material.emission < 0f)
                    throw new InvalidDataException("Invalid PBR scalar for style " + material.id);
                if (material.frames < 0 || material.frames > MaxFrames)
                    throw new InvalidDataException("frames must be 0..1024 for style " + material.id);
                if(material.textureRotation<0||material.textureRotation>23)throw new InvalidDataException("textureRotation must be 0..23 for style "+material.id);
                if(!Finite(material.textureScale)||material.textureScale<0||material.textureScale>256)throw new InvalidDataException("textureScale must be 0..256 for style "+material.id);
                if (material.frames > 1 && (!Finite(material.frameTime) || material.frameTime <= 0f))
                    throw new InvalidDataException("Animated style needs a positive frameTime: " + material.id);
                if (material.textures != null && material.textures.Count != 0 && material.textures.Count != 6)
                    throw new InvalidDataException("textures must be empty or exactly six faces for style " + material.id);
            }
            document.materials.Sort((left, right) => left.id.CompareTo(right.id));
            return document.materials;
        }

        static bool Finite(float value)
        {
            return !float.IsNaN(value) && !float.IsInfinity(value);
        }

        static List<PreparedMaterial> Prepare(List<CatalogMaterial> materials, string root)
        {
            var cache = new Dictionary<string, SourceTexture>(StringComparer.OrdinalIgnoreCase);
            var decodedPixels = 0L;
            var prepared = new List<PreparedMaterial>(materials.Count);
            foreach (var material in materials)
            {
                SourceTexture[] sources = null;
                var declaredFrames = material.frames;
                var frames = declaredFrames > 0 ? declaredFrames : 0;
                if (material.textures != null && material.textures.Count == 6)
                {
                    sources = new SourceTexture[6];
                    for (var face = 0; face < 6; face++)
                    {
                        var path = ResolveTexturePath(root, material.textures[face]);
                        if (!cache.TryGetValue(path, out var source))
                        {
                            source = LoadSourceTexture(path);
                            var pixels = (long)source.Width * source.Height;
                            if (decodedPixels + pixels > 64L * 1024L * 1024L)
                                throw new InvalidDataException("Catalog decoded texture pixel budget exceeded at " + path);
                            decodedPixels += pixels;
                            cache.Add(path, source);
                        }
                        if (declaredFrames > 0 && source.Frames != declaredFrames)
                            throw new InvalidDataException(
                                "frame count mismatch for style " + material.id + ", face " + face + ": manifest " + declaredFrames + ", PNG " + source.Frames);
                        if (frames > 0 && source.Frames != frames)
                            throw new InvalidDataException(
                                "face frame count mismatch for style " + material.id + ", face " + face + ": expected " + frames + ", PNG " + source.Frames);
                        frames = source.Frames;
                        sources[face] = source;
                    }
                }
                if (frames <= 0) frames = 1;
                if (frames > 1 && (!Finite(material.frameTime) || material.frameTime <= 0f))
                    throw new InvalidDataException("Animated style needs a positive frameTime: " + material.id);

                var flags = 0;
                if (sources != null) flags |= HasTextureFlag;
                if (material.renderMode == "transparent") flags |= TransparentFlag;
                if (material.renderMode == "transparent" && material.SurfaceMasks) flags |= 32;
                if (material.renderMode == "cutout") flags |= CutoutFlag;
                if (string.Equals(material.family, "water", StringComparison.OrdinalIgnoreCase)) flags |= WaterFlag;
                if (string.Equals(material.family, "cloud", StringComparison.OrdinalIgnoreCase)) flags |= CloudFlag;
                prepared.Add(new PreparedMaterial
                {
                    Material = material,
                    Frames = frames,
                    Textures = sources,
                    Flags = flags
                });
            }
            return prepared;
        }

        static string ResolveTexturePath(string root, string url)
        {
            if (string.IsNullOrWhiteSpace(url))
                throw new InvalidDataException("Empty catalog texture path");
            var relative = Uri.UnescapeDataString(url).Replace('\\', '/');
            if (relative.StartsWith("/", StringComparison.Ordinal)) relative = relative.Substring(1);
            if (!relative.StartsWith("artifacts/", StringComparison.Ordinal))
                throw new InvalidDataException("Catalog textures must use artifacts/<hash>.png: " + url);
            var parts = relative.Split('/');
            if (parts.Any(part => part == ".." || part == "." || part.Length == 0))
                throw new InvalidDataException("Unsafe catalog texture path: " + url);
            if (!relative.EndsWith(".png", StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("Only PNG catalog artifacts are accepted: " + url);
            var rootFull = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar) + Path.DirectorySeparatorChar;
            var full = Path.GetFullPath(Path.Combine(root, relative.Replace('/', Path.DirectorySeparatorChar)));
            if (!full.StartsWith(rootFull, StringComparison.OrdinalIgnoreCase))
                throw new InvalidDataException("Catalog texture escapes source root: " + url);
            if (!File.Exists(full))
                throw new FileNotFoundException("Catalog texture missing", full);
            if ((File.GetAttributes(full) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("Catalog texture may not be a symlink/reparse point: " + url);
            return full;
        }

        static SourceTexture LoadSourceTexture(string path)
        {
            var bytes = File.ReadAllBytes(path);
            if (bytes.Length == 0 || bytes.Length > 32 * 1024 * 1024)
                throw new InvalidDataException("Catalog PNG exceeds 32 MiB: " + path);
            if (bytes.Length < 24 || bytes[0] != 0x89 || bytes[1] != 0x50 || bytes[2] != 0x4e || bytes[3] != 0x47)
                throw new InvalidDataException("Not a PNG: " + path);
            var width = ReadUInt32BigEndian(bytes, 16);
            var height = ReadUInt32BigEndian(bytes, 20);
            if (width == 0 || height == 0 || width > 16384 || height > 16384 ||
                (ulong)width * height > 16UL * 1024UL * 1024UL)
                throw new InvalidDataException("Catalog PNG dimensions are out of range: " + path);
            if (height % width != 0)
                throw new InvalidDataException("Catalog PNG must be a vertical square-frame strip: " + path);
            var frames = (int)(height / width);
            if (frames < 1 || frames > MaxFrames)
                throw new InvalidDataException("Catalog PNG frame count is out of range: " + path);

            var texture = new Texture2D(2, 2, TextureFormat.RGBA32, false, false);
            try
            {
                if (!texture.LoadImage(bytes))
                    throw new InvalidDataException("Unity could not decode catalog PNG: " + path);
                if (texture.width != width || texture.height != height)
                    throw new InvalidDataException("PNG header and decoded size disagree: " + path);
                return new SourceTexture
                {
                    Path = path,
                    Width = (int)width,
                    Height = (int)height,
                    Frames = frames,
                    Pixels = texture.GetPixels32()
                };
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(texture);
            }
        }

        static uint ReadUInt32BigEndian(byte[] bytes, int offset)
        {
            return ((uint)bytes[offset] << 24) |
                   ((uint)bytes[offset + 1] << 16) |
                   ((uint)bytes[offset + 2] << 8) |
                   bytes[offset + 3];
        }

        static AtlasLayout Pack(List<PreparedMaterial> materials)
        {
            if(LayeredOnly)return new AtlasLayout{Width=1,Height=1,Items=materials.Select(p=>new PackedMaterial{Prepared=p}).ToList()};
            var maxTextureSize = Mathf.Min(16384, Mathf.Max(256, SystemInfo.maxTextureSize));
            var atlasWidth = Mathf.Min(8192, maxTextureSize);
            if (atlasWidth < CellPixels * 6)
                throw new InvalidOperationException("GPU max texture size is too small for six 32px faces.");

            var layout = new AtlasLayout
            {
                Width = 1,
                Height = 1,
                Items = new List<PackedMaterial>(materials.Count)
            };
            var x = 0;
            var y = 0;
            var rowHeight = 0;
            var usedWidth = 0;
            var usedHeight = 0;
            var blockWidth = CellPixels * 6;
            var reused=new Dictionary<string,PackedMaterial>();
            foreach (var material in materials)
            {
                var item = new PackedMaterial { Prepared = material };
                if (material.Textures != null)
                {
                    var identity=material.Frames+":"+string.Join("|",material.Textures.Select(t=>t.Path));
                    if(reused.TryGetValue(identity,out var prior)){item.OffsetX=prior.OffsetX;item.OffsetY=prior.OffsetY;layout.Items.Add(item);continue;}
                    var blockHeight = CellPixels * Mathf.Max(1, material.Frames);
                    if (x + blockWidth > atlasWidth)
                    {
                        x = 0;
                        y += rowHeight;
                        rowHeight = 0;
                    }
                    if (y + blockHeight > maxTextureSize)
                        throw new InvalidOperationException(
                            "Catalog atlas exceeds " + maxTextureSize + "px at style " + material.Material.id +
                            "; reduce cell size, split material sets, or trim animation frames.");
                    item.OffsetX = x;
                    item.OffsetY = y;
                    reused[identity]=item;
                    x += blockWidth;
                    rowHeight = Mathf.Max(rowHeight, blockHeight);
                    usedWidth = Mathf.Max(usedWidth, x);
                    usedHeight = Mathf.Max(usedHeight, y + blockHeight);
                }
                layout.Items.Add(item);
            }
            if (usedWidth > 0)
            {
                layout.Width = usedWidth;
                layout.Height = usedHeight;
            }
            return layout;
        }

        static void WriteAtlas(AtlasLayout layout)
        {
            var pixels = new Color32[layout.Width * layout.Height];
            foreach (var item in layout.Items.Where(item => !LayeredOnly && item.Prepared.Textures != null))
            {
                for (var face = 0; face < 6; face++)
                {
                    var source = item.Prepared.Textures[face];
                    for (var frame = 0; frame < item.Prepared.Frames; frame++)
                    {
                        var frameTop = source.Height - (frame + 1) * source.Width;
                        for (var y = 0; y < CellPixels; y++)
                        {
                            var sourceY = frameTop + (y * source.Width) / CellPixels;
                            for (var x = 0; x < CellPixels; x++)
                            {
                                var sourceX = (x * source.Width) / CellPixels;
                                var destinationX = item.OffsetX + face * CellPixels + x;
                                var destinationY = item.OffsetY + frame * CellPixels + y;
                                pixels[destinationY * layout.Width + destinationX] =
                                    source.Pixels[sourceY * source.Width + sourceX];
                            }
                        }
                    }
                }
            }

            if(LayeredOnly)pixels[0]=new Color32(255,255,255,255);
            var texture = new Texture2D(layout.Width, layout.Height, TextureFormat.RGBA32, false, false);
            try
            {
                texture.SetPixels32(pixels);
                texture.Apply(false, false);
                var path = Path.GetFullPath(Path.Combine(ProjectRoot, AtlasAssetPath));
                Directory.CreateDirectory(Path.GetDirectoryName(path));
                File.WriteAllBytes(path, texture.EncodeToPNG());
            }
            finally
            {
                UnityEngine.Object.DestroyImmediate(texture);
            }
            AssetDatabase.ImportAsset(AtlasAssetPath, ImportAssetOptions.ForceUpdate);
            var importer = AssetImporter.GetAtPath(AtlasAssetPath) as TextureImporter;
            if (!importer)
                throw new InvalidOperationException("Unable to configure catalog atlas importer.");
            importer.textureType = TextureImporterType.Default;
            importer.sRGBTexture = true;
            importer.alphaSource = TextureImporterAlphaSource.FromInput;
            importer.alphaIsTransparency = true;
            importer.mipmapEnabled = false;
            importer.filterMode = FilterMode.Point;
            importer.wrapMode = TextureWrapMode.Clamp;
            importer.textureCompression = TextureImporterCompression.Uncompressed;
            importer.npotScale = TextureImporterNPOTScale.None;
            importer.SaveAndReimport();
        }

        static Texture2DArray WriteLayers(AtlasLayout layout)
        {
            LayerBanks=new Texture2DArray[4];FaceMap=null;UniqueSlices=0;
            LayerBankSize=Mathf.Min(2048,SystemInfo.maxTextureArraySlices);
            if(!SystemInfo.supports2DArrayTextures||LayerBankSize<1)return null;
            var groups=layout.Items.Where(i=>i.Prepared.Textures!=null).GroupBy(i=>i.Prepared.Frames+":"+string.Join("|",i.Prepared.Textures.Select(t=>t.Path))).ToArray();
            if(groups.Length==0)return null;
            var slices=new List<Color32[]>();var hashes=new Dictionary<string,int>();var cache=new Dictionary<string,int>();var map=new List<int>();
            foreach(var group in groups){
                foreach(var member in group){member.LayerBase=map.Count;member.Prepared.Flags|=64;}
                var item=group.First();
                for(int frame=0;frame<item.Prepared.Frames;frame++)for(int face=0;face<6;face++){
                    var source=item.Prepared.Textures[face];var key=source.Path+":"+frame;
                    if(!cache.TryGetValue(key,out int address)){
                        var pixels=new Color32[CellPixels*CellPixels];var top=source.Height-(frame+1)*source.Width;
                        for(int y=0;y<CellPixels;y++)for(int x=0;x<CellPixels;x++)pixels[y*CellPixels+x]=source.Pixels[(top+y*source.Width/CellPixels)*source.Width+x*source.Width/CellPixels];
                        var raw=new byte[pixels.Length*4];for(int i=0;i<pixels.Length;i++){raw[i*4]=pixels[i].r;raw[i*4+1]=pixels[i].g;raw[i*4+2]=pixels[i].b;raw[i*4+3]=pixels[i].a;}
                        string hash;using(var sha=System.Security.Cryptography.SHA256.Create())hash=Convert.ToBase64String(sha.ComputeHash(raw));
                        if(!hashes.TryGetValue(hash,out address)){address=slices.Count;hashes.Add(hash,address);slices.Add(pixels);}
                        cache.Add(key,address);
                    }
                    map.Add(address);
                }
            }
            UniqueSlices=slices.Count;
            if(slices.Count>LayerBankSize*4||(long)slices.Count*CellPixels*CellPixels*4*4/3>64*1024*1024)throw new InvalidOperationException("Unique texture slices exceed four banks / 64MiB; select fewer materials or a lower cell resolution.");
            for(int bank=0;bank*LayerBankSize<slices.Count;bank++){
                int count=Mathf.Min(LayerBankSize,slices.Count-bank*LayerBankSize);
                var array=new Texture2DArray(CellPixels,CellPixels,count,TextureFormat.RGBA32,true,false){name="CatalogLayers"+bank,filterMode=FilterMode.Trilinear,wrapMode=TextureWrapMode.Repeat,anisoLevel=8};
                for(int i=0;i<count;i++)array.SetPixels32(slices[bank*LayerBankSize+i],i,0);array.Apply(true,false);
                var path=bank==0?LayersAssetPath:OutputRoot+"/CatalogLayers"+bank+".asset";
                var existing=AssetDatabase.LoadAssetAtPath<Texture2DArray>(path);
                if(existing){EditorUtility.CopySerialized(array,existing);UnityEngine.Object.DestroyImmediate(array);EditorUtility.SetDirty(existing);array=existing;}else AssetDatabase.CreateAsset(array,path);
                LayerBanks[bank]=array;
            }
            int width=256,height=Mathf.Max(1,(map.Count+255)/256);var mapping=new Texture2D(width,height,TextureFormat.RGBA32,false,true){name="CatalogFaceMap",filterMode=FilterMode.Point,wrapMode=TextureWrapMode.Clamp};
            var values=new Color32[width*height];for(int i=0;i<map.Count;i++)values[i]=new Color32((byte)map[i],(byte)(map[i]>>8),0,255);mapping.SetPixels32(values);mapping.Apply(false,false);
            var oldMap=AssetDatabase.LoadAssetAtPath<Texture2D>(OutputRoot+"/CatalogFaceMap.asset");
            if(oldMap){EditorUtility.CopySerialized(mapping,oldMap);UnityEngine.Object.DestroyImmediate(mapping);EditorUtility.SetDirty(oldMap);mapping=oldMap;}else AssetDatabase.CreateAsset(mapping,OutputRoot+"/CatalogFaceMap.asset");FaceMap=mapping;
            return LayerBanks[0];
        }

        static void WriteMetadata(AtlasLayout layout)
        {
            // RGBAFloat is a linear data texture, not a color texture. Use
            // SetPixelData so pixel offsets/counts above 1.0 are not color-clamped.
            var pixels = new float[MetadataTexelCount * 4];
            foreach (var item in layout.Items)
            {
                var index = item.Prepared.Material.id * 16;
                var material = item.Prepared.Material;
                pixels[index] = item.OffsetX;
                pixels[index + 1] = item.OffsetY;
                pixels[index + 2] = item.Prepared.Frames;
                pixels[index + 3] = Mathf.Max(0.001f, material.frameTime);
                pixels[index + 4] = material.color[0];
                pixels[index + 5] = material.color[1];
                pixels[index + 6] = material.color[2];
                pixels[index + 7] = material.opacity;
                pixels[index + 8] = material.metallic;
                pixels[index + 9] = material.roughness;
                pixels[index + 10] = material.emission;
                pixels[index + 11] = item.Prepared.Flags;
                pixels[index + 12] = CellPixels;
                pixels[index + 13] = item.LayerBase;
                // Zero remains legacy one-cell tiling. One means stretch.
                pixels[index + 14] = material.textureScale + 1f;
                pixels[index + 15] = material.textureRotation;
            }

            // Preserve the asset GUID so existing material references survive rebuilds.
            var texture = AssetDatabase.LoadAssetAtPath<Texture2D>(MetadataAssetPath);
            var created = !texture;
            if (created)
                texture = new Texture2D(MetadataWidth, MetadataHeight, TextureFormat.RGBAFloat, false, true);
            else
            {
                if (texture.format != TextureFormat.RGBAFloat || texture.isDataSRGB)
                    throw new InvalidDataException("Existing catalog metadata must be linear RGBAFloat.");
                if (!texture.Reinitialize(MetadataWidth, MetadataHeight, TextureFormat.RGBAFloat, false))
                    throw new InvalidOperationException("Unable to resize catalog metadata.");
            }
            texture.name = "CatalogMetadata";
            texture.filterMode = FilterMode.Point;
            texture.wrapMode = TextureWrapMode.Clamp;
            texture.anisoLevel = 0;
            texture.SetPixelData(pixels, 0);
            texture.Apply(false, false);
            if (created) AssetDatabase.CreateAsset(texture, MetadataAssetPath);
            else EditorUtility.SetDirty(texture);
            AssetDatabase.SaveAssets();
        }

        static string ProjectRoot
        {
            get { return Path.GetFullPath(Path.Combine(Application.dataPath, "..")); }
        }

        static void EnsureFolder(string path)
        {
            var current = "Assets";
            foreach (var part in path.Split('/').Skip(1))
            {
                var next = current + "/" + part;
                if (!AssetDatabase.IsValidFolder(next))
                    AssetDatabase.CreateFolder(current, part);
                current = next;
            }
        }

        static void WriteReport(string name, string content)
        {
            var directory = Path.Combine(ProjectRoot, "Docs", "Materials", OutputRoot == DefaultOutputRoot ? "" : "diagnostics");
            Directory.CreateDirectory(directory);
            File.WriteAllText(Path.Combine(directory, name), content + Environment.NewLine);
        }

        [JsonObject(MemberSerialization.OptIn)]
        public sealed class CatalogDocument
        {
            [JsonProperty("format")] public string format;
            [JsonProperty("materials")] public List<CatalogMaterial> materials;
            [JsonProperty("capabilities")] public List<string> capabilities;
        }

        [JsonObject(MemberSerialization.OptIn)]
        public sealed class CatalogMaterial
        {
            [JsonIgnore] public bool SurfaceMasks;
            [JsonProperty("id")] public int id;
            [JsonProperty("uid")] public string uid;
            [JsonProperty("name")] public string name;
            [JsonProperty("family")] public string family;
            [JsonProperty("color")] public float[] color;
            [JsonProperty("metallic")] public float metallic;
            [JsonProperty("roughness")] public float roughness;
            [JsonProperty("opacity")] public float opacity;
            [JsonProperty("emission")] public float emission;
            [JsonProperty("renderMode")] public string renderMode;
            [JsonProperty("textures")] public List<string> textures;
            [JsonProperty("textureScale")] public float textureScale = 1f;
            [JsonProperty("textureRotation")] public int textureRotation;
            [JsonProperty("frames")] public int frames;
            [JsonProperty("frameTime")] public float frameTime;
            [JsonProperty("pack")] public string pack;
            [JsonProperty("source")] public string source;
        }

        sealed class SourceTexture
        {
            public string Path;
            public int Width;
            public int Height;
            public int Frames;
            public Color32[] Pixels;
        }

        sealed class PreparedMaterial
        {
            public CatalogMaterial Material;
            public int Frames;
            public SourceTexture[] Textures;
            public int Flags;
        }

        sealed class PackedMaterial
        {
            public PreparedMaterial Prepared;
            public int OffsetX;
            public int OffsetY;
            public int LayerBase;
        }

        sealed class AtlasLayout
        {
            public int Width;
            public int Height;
            public List<PackedMaterial> Items;
        }
    }
}
