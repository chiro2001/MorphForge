using System;
using System.IO;
using System.Linq;
using UnityEditor;
using Newtonsoft.Json.Linq;
using VRC.SDKBase.Editor;
using VRC.SDK3A.Editor;
public static class Mf4UploadWake
{
    public static string Run()
    {
        if(!VRCSdkControlPanel.TryGetBuilder(out IVRCSdkAvatarBuilderApi b)||b.BuildState!=SdkBuildState.Idle||b.UploadState!=SdkUploadState.Idle)throw new Exception("SDK must be idle; never invoke twice.");
        var callbacks=EditorApplication.delayCall?.GetInvocationList().Where(d=>d.Method.DeclaringType.FullName.StartsWith("Mf4Publish+")).ToArray()??new Delegate[0];
        if(callbacks.Length!=1)throw new Exception("Expected exactly one unexecuted MF4 upload callback, found "+callbacks.Length);
        var path=Path.Combine(System.IO.Directory.GetParent(UnityEngine.Application.dataPath).FullName,"Docs/MF4/upload.json");var s=JObject.Parse(File.ReadAllText(path));
        if((string)s["status"]!="running"||(int)s["versionBefore"]!=11)throw new Exception("Unexpected queued operation.");
        EditorApplication.delayCall-=(EditorApplication.CallbackFunction)callbacks[0];
        s["phase"]="sdk-invoked";s["invokedUtc"]=DateTime.UtcNow.ToString("O");s["unfocusedDelayCallRecovered"]=true;File.WriteAllText(path,s.ToString());
        callbacks[0].DynamicInvoke();
        return new JObject{["success"]=true,["queuedCallbackRemoved"]=true,["operation"]=s["operation"],["buildState"]=b.BuildState.ToString(),["uploadState"]=b.UploadState.ToString()}.ToString();
    }
}
