using System;
using MorphForge.Editor;

public static class FullFTEntry
{
    public static string Inspect()
    {
        return FullFTBuild.Inspect();
    }

    public static string DependencyPlan()
    {
        return Newtonsoft.Json.JsonConvert.SerializeObject(
            FullFTDependencies.Plan(),
            Newtonsoft.Json.Formatting.Indented);
    }

    public static string ImportDependencies()
    {
        return Newtonsoft.Json.JsonConvert.SerializeObject(
            FullFTDependencies.ImportMinimal(),
            Newtonsoft.Json.Formatting.Indented);
    }

    public static string InstallVrcFury()
    {
        return Newtonsoft.Json.JsonConvert.SerializeObject(
            FullFTDependencies.InstallVrcFury(),
            Newtonsoft.Json.Formatting.Indented);
    }

    public static string EnsureProfile()
    {
        return FullFTBuild.EnsureProfile();
    }

    public static string ResetProfile()
    {
        return FullFTBuild.ResetProfile();
    }

    public static string BuildTestAvatar()
    {
        return FullFTBuild.BuildTestAvatar();
    }

    public static string JointBuild()
    {
        return FullFTBuild.JointBuild();
    }

    public static string CreateScene()
    {
        return FullFTBuild.CreateScene();
    }
}
