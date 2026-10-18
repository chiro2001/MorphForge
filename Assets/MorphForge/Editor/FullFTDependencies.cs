using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using UnityEditor;

namespace MorphForge.Editor
{
    /// <summary>
    /// Read-only planning plus the explicitly invoked local dependency import for
    /// the minimal FullFT injection. No base avatar, outfits, sleep system or
    /// generated VRCFury build directory is copied.
    /// </summary>
    public static class FullFTDependencies
    {
        public const string SourceAssetsRoot =
            @"D:\Projects\VRChatProjects2\Nemesis\Assets";

        private const string VrcFuryPackageId = "com.vrcfury.vrcfury";

        private static readonly string[] SourceDirectories =
        {
            "MoeWhite/VRCFT/adjerry91.vrcft.templates",
            "MoeWhite/VRCFT/Anim",
            "MoeWhite/VRCFT/EyeLinkEar",
            "MoeWhite/VRCFT/FBX",
            "MoeWhite/VRCFT/Menu",
            "MoeWhite/VRCFT/Prefab",
            "MoeWhite/VRCFT/Runtime",
        };

        private static readonly string[] SourceFiles =
        {
            "MoeWhite/_Empty.anim",
        };

        private static readonly string[] RequiredPackages =
        {
            "com.vrcfury.vrcfury",
            "nadena.dev.modular-avatar",
            "nadena.dev.ndmf",
            "com.vrchat.avatars",
        };

        public sealed class Candidate
        {
            public string relativePath;
            public string sourcePath;
            public string targetPath;
            public string guid;
            public bool isMeta;
            public bool isDirectory;
            public string sourceHash;
        }

        public static object Plan()
        {
            var sourceRoot = Normalize(SourceAssetsRoot);
            var targetAssets = Normalize(Path.Combine(FullFTBudgetUtil.ProjectRoot, "Assets"));
            var targetGuidMap = BuildTargetGuidMap();
            var candidates = EnumerateCandidates(sourceRoot, targetAssets);
            var missingSource = candidates.Where(item => item.isDirectory
                ? !Directory.Exists(item.sourcePath)
                : !File.Exists(item.sourcePath)).ToList();
            var conflicts = new List<object>();
            var add = new List<object>();
            var unchanged = new List<object>();
            var packageStatus = RequiredPackages.ToDictionary(
                package => package,
                package => File.Exists(Path.Combine(
                    FullFTBudgetUtil.ProjectRoot, "Packages", package, "package.json")));

            foreach (var duplicate in candidates
                         .Where(item => !item.isDirectory && !string.IsNullOrEmpty(item.guid))
                         .GroupBy(item => item.guid, StringComparer.Ordinal)
                         .Where(group => group.Count() > 1))
            {
                conflicts.Add(new
                {
                    type = "source-guid-duplicate",
                    guid = duplicate.Key,
                    paths = duplicate.Select(item => item.sourcePath).ToArray(),
                });
            }

            foreach (var candidate in candidates.Where(item => item.isDirectory
                         ? Directory.Exists(item.sourcePath)
                         : File.Exists(item.sourcePath)))
            {
                if (!string.IsNullOrEmpty(candidate.guid) &&
                    targetGuidMap.TryGetValue(candidate.guid, out var existingPath) &&
                    !string.Equals(existingPath, candidate.targetPath, StringComparison.OrdinalIgnoreCase))
                {
                    conflicts.Add(new
                    {
                        type = "guid-at-different-path",
                        candidate.guid,
                        source = candidate.sourcePath,
                        target = candidate.targetPath,
                        existing = existingPath,
                    });
                    continue;
                }

                var targetExists = candidate.isDirectory
                    ? Directory.Exists(candidate.targetPath)
                    : File.Exists(candidate.targetPath);
                if (!targetExists)
                {
                    add.Add(new
                    {
                        candidate.relativePath,
                        candidate.guid,
                        candidate.isDirectory,
                        bytes = candidate.isDirectory
                            ? 0
                            : new FileInfo(candidate.sourcePath).Length,
                    });
                    continue;
                }

                if (candidate.isDirectory)
                {
                    unchanged.Add(new { candidate.relativePath, candidate.guid });
                    continue;
                }

                var targetHash = Sha256(candidate.targetPath);
                if (!string.Equals(candidate.sourceHash, targetHash, StringComparison.OrdinalIgnoreCase))
                {
                    conflicts.Add(new
                    {
                        type = "target-content-differs",
                        candidate.relativePath,
                        sourceHash = candidate.sourceHash,
                        targetHash,
                    });
                    continue;
                }

                unchanged.Add(new { candidate.relativePath, candidate.guid });
            }

            var missingPackages = packageStatus
                .Where(pair => !pair.Value)
                .Select(pair => pair.Key)
                .ToArray();
            var success = missingSource.Count == 0 &&
                          conflicts.Count == 0 &&
                          missingPackages.Length == 0;

            var report = new
            {
                success,
                readOnly = true,
                sourceRoot,
                targetAssets,
                sourceDirectories = SourceDirectories,
                sourceFiles = SourceFiles,
                packageStatus,
                missingPackages,
                summary = new
                {
                    candidates = candidates.Count,
                    add = add.Count,
                    unchanged = unchanged.Count,
                    missingSource = missingSource.Count,
                    conflicts = conflicts.Count,
                    addBytes = candidates
                        .Where(item => !item.isDirectory &&
                                       File.Exists(item.sourcePath) &&
                                       !File.Exists(item.targetPath))
                        .Sum(item => new FileInfo(item.sourcePath).Length),
                },
                missingSource = missingSource.Select(item => item.sourcePath).ToArray(),
                conflicts,
                add,
                unchanged,
                excluded = new[]
                {
                    "Assets/Nemesis model files",
                    "Ruruku's dress-up folder",
                    "VGC/Gimmicks_MA and sleep system",
                    "MinMinMart Supine generated controller",
                    "MoeWhite/VRCFT/Patcher",
                    "MoeWhite/VRCFT/Editor",
                    "Packages/com.vrcfury.temp",
                },
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/dependency-plan.json", report);
            return report;
        }

        /// <summary>
        /// Installs the exact embedded VRCFury package already used by the source
        /// project. Call only after the Unity long test ends.
        /// </summary>
        public static object InstallVrcFury()
        {
            if (UnityEditor.EditorApplication.isPlaying ||
                UnityEditor.EditorApplication.isCompiling)
                throw new InvalidOperationException(
                    "VRCFury installation requires idle Edit Mode.");

            var sourcePackageRoot = Path.GetDirectoryName(Normalize(SourceAssetsRoot)) ??
                                    throw new InvalidOperationException(
                                        "Cannot resolve the source project root.");
            var source = Path.Combine(
                sourcePackageRoot, "Packages", VrcFuryPackageId);
            var target = Path.Combine(
                FullFTBudgetUtil.ProjectRoot, "Packages", VrcFuryPackageId);
            if (!Directory.Exists(source))
                throw new InvalidOperationException(
                    $"Source VRCFury package is missing at '{source}'.");

            var sourceVersion = ReadPackageVersion(source);
            if (Directory.Exists(target))
            {
                var targetVersion = ReadPackageVersion(target);
                return new
                {
                    success = sourceVersion == targetVersion,
                    action = "already-present",
                    sourceVersion,
                    targetVersion,
                    target,
                };
            }

            var files = Directory.GetFiles(source, "*", SearchOption.AllDirectories);
            foreach (var file in files)
            {
                var relative = Path.GetRelativePath(source, file);
                var destination = Path.Combine(target, relative);
                Directory.CreateDirectory(Path.GetDirectoryName(destination) ??
                                          throw new InvalidOperationException(
                                              "No destination directory."));
                File.Copy(file, destination, false);
            }
            AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
            var result = new
            {
                success = true,
                action = "copied-embedded-package",
                package = VrcFuryPackageId,
                version = sourceVersion,
                copied = files.Length,
                source,
                target,
                generatedBuildDirectoryCopied = false,
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/vrcfury-install.json", result);
            return result;
        }

        /// <summary>
        /// Performs the planned copy. Call only after the Unity long test ends and
        /// the main agent explicitly permits resource import.
        /// </summary>
        public static object ImportMinimal()
        {
            var plan = Plan();
            var success = (bool)plan.GetType().GetProperty("success").GetValue(plan);
            if (!success)
                throw new InvalidOperationException(
                    "Dependency plan has missing packages, source files or conflicts. " +
                    "Read Docs/FullFT/dependency-plan.json before importing.");

            var sourceRoot = Normalize(SourceAssetsRoot);
            var targetAssets = Normalize(Path.Combine(FullFTBudgetUtil.ProjectRoot, "Assets"));
            var candidates = EnumerateCandidates(sourceRoot, targetAssets)
                .Where(item => File.Exists(item.sourcePath))
                .ToArray();

            var copied = 0;
            var reused = 0;
            AssetDatabase.StartAssetEditing();
            try
            {
                foreach (var candidate in candidates)
                {
                    var directory = Path.GetDirectoryName(candidate.targetPath) ??
                                    throw new InvalidOperationException("No target directory.");
                    Directory.CreateDirectory(directory);
                    if (candidate.isDirectory)
                    {
                        Directory.CreateDirectory(candidate.targetPath);
                        continue;
                    }
                    if (File.Exists(candidate.targetPath))
                    {
                        if (!string.Equals(
                                Sha256(candidate.sourcePath),
                                Sha256(candidate.targetPath),
                                StringComparison.OrdinalIgnoreCase))
                            throw new InvalidOperationException(
                                $"Target changed during import: {candidate.relativePath}");
                        reused++;
                        continue;
                    }

                    File.Copy(candidate.sourcePath, candidate.targetPath, false);
                    copied++;
                }
            }
            finally
            {
                AssetDatabase.StopAssetEditing();
            }

            AssetDatabase.Refresh(ImportAssetOptions.ForceSynchronousImport);
            var report = new
            {
                success = true,
                copied,
                reused,
                sourceRoot,
                targetAssets,
                excludedBaseModel = true,
                excludedOutfits = true,
                excludedSleep = true,
                excludedGeneratedVrcFury = true,
            };
            FullFTBudgetUtil.WriteReport("Docs/FullFT/dependency-import.json", report);
            return report;
        }

        private static List<Candidate> EnumerateCandidates(string sourceRoot, string targetAssets)
        {
            var result = new List<Candidate>();
            var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

            void AddPath(string relative)
            {
                relative = relative.Replace('\\', '/').TrimEnd('/');
                if (!seen.Add(relative)) return;
                var source = Path.Combine(sourceRoot, relative.Replace('/', Path.DirectorySeparatorChar));
                var target = Path.Combine(targetAssets, relative.Replace('/', Path.DirectorySeparatorChar));
                var meta = source + ".meta";
                result.Add(new Candidate
                {
                    relativePath = relative,
                    sourcePath = source,
                    targetPath = target,
                    guid = null,
                    isMeta = false,
                    isDirectory = Directory.Exists(source),
                    sourceHash = File.Exists(source) ? Sha256(source) : null,
                });
                if (File.Exists(meta))
                {
                    AddMeta(relative + ".meta", meta, target + ".meta");
                }
            }

            void AddMeta(string relative, string source, string target)
            {
                relative = relative.Replace('\\', '/');
                if (!seen.Add(relative)) return;
                result.Add(new Candidate
                {
                    relativePath = relative,
                    sourcePath = source,
                    targetPath = target,
                    guid = ReadGuid(source),
                    isMeta = true,
                    isDirectory = false,
                    sourceHash = File.Exists(source) ? Sha256(source) : null,
                });
            }

            AddPath("MoeWhite");
            AddPath("MoeWhite/VRCFT");
            foreach (var directory in SourceDirectories)
            {
                AddPath(directory);
                var full = Path.Combine(sourceRoot, directory.Replace('/', Path.DirectorySeparatorChar));
                if (!Directory.Exists(full)) continue;
                foreach (var subdirectory in Directory.GetDirectories(full, "*", SearchOption.AllDirectories))
                {
                    var relative = Path.GetRelativePath(sourceRoot, subdirectory).Replace('\\', '/');
                    AddPath(relative);
                }
                foreach (var file in Directory.GetFiles(full, "*", SearchOption.AllDirectories))
                {
                    if (file.EndsWith(".meta", StringComparison.OrdinalIgnoreCase)) continue;
                    AddPath(Path.GetRelativePath(sourceRoot, file).Replace('\\', '/'));
                }
            }
            foreach (var file in SourceFiles) AddPath(file);
            return result;
        }

        private static Dictionary<string, string> BuildTargetGuidMap()
        {
            var map = new Dictionary<string, string>(StringComparer.Ordinal);
            var projectRoot = FullFTBudgetUtil.ProjectRoot;
            foreach (var rootName in new[] { "Assets", "Packages" })
            {
                var root = Path.Combine(projectRoot, rootName);
                if (!Directory.Exists(root)) continue;
                foreach (var meta in Directory.GetFiles(root, "*.meta", SearchOption.AllDirectories))
                {
                    var guid = ReadGuid(meta);
                    if (string.IsNullOrEmpty(guid) || map.ContainsKey(guid)) continue;
                    map[guid] = Normalize(meta.Substring(0, meta.Length - 5));
                }
            }
            return map;
        }

        private static string ReadGuid(string metaPath)
        {
            if (!File.Exists(metaPath)) return null;
            foreach (var line in File.ReadLines(metaPath).Take(20))
            {
                if (line.StartsWith("guid: ", StringComparison.Ordinal))
                    return line.Substring(6).Trim();
            }
            return null;
        }

        private static string ReadPackageVersion(string root)
        {
            var packageJson = Path.Combine(root, "package.json");
            if (!File.Exists(packageJson)) return null;
            try
            {
                var json = Newtonsoft.Json.Linq.JObject.Parse(File.ReadAllText(packageJson));
                return (string)json["version"];
            }
            catch
            {
                return null;
            }
        }

        private static string Sha256(string path)
        {
            using var stream = File.OpenRead(path);
            using var sha = SHA256.Create();
            return BitConverter.ToString(sha.ComputeHash(stream)).Replace("-", string.Empty);
        }

        private static string Normalize(string path)
        {
            return Path.GetFullPath(path).Replace('\\', '/').TrimEnd('/');
        }
    }
}
