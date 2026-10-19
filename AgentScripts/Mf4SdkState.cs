using System;
using UnityEditor;
using VRC.SDKBase.Editor;
using VRC.SDK3A.Editor;
using Newtonsoft.Json.Linq;
public static class Mf4SdkState
{
    public static string Run()
    {
        var report=new JObject{["timeUtc"]=DateTime.UtcNow.ToString("O"),["playing"]=EditorApplication.isPlaying,["compiling"]=EditorApplication.isCompiling,["updating"]=EditorApplication.isUpdating};
        if(VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi b)){report["buildState"]=b.BuildState.ToString();report["uploadState"]=b.UploadState.ToString();}
        return report.ToString();
    }
}
