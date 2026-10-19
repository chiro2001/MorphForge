using System;
using System.IO;
using System.Linq;
using System.Collections.Generic;
using System.Threading.Tasks;
using UnityEngine;
using VRC.SDKBase.Editor.Api;
using MorphForge.Editor;
using Newtonsoft.Json.Linq;
public static class Mf4Analysis
{
    public static async Task<string> Run()
    {
        var avatar=await VRCApi.GetAvatar(Mf4AvatarBuild.AvatarId,forceRefresh:true);
        if(avatar.AuthorId!="usr_c0dd064e-ac42-47a7-a17e-da6540382c55"||avatar.ReleaseStatus!="private"||avatar.Name!="MorphForge MF4 16K")throw new Exception("Unexpected Avatar identity/owner.");
        var url=avatar.GetLatestAssetUrlForCurrentPlatform();int start=url.IndexOf("/file_",StringComparison.Ordinal);if(start<0)throw new Exception("Missing asset file identity.");var parts=url.Substring(start+1).Split('/');string file=parts[0],version=parts[1];
        var results=new JArray();bool terminal=true,passed=true;
        foreach(var suffix in new[]{"","/standard","/security"})
        {
            string endpoint="analysis/"+file+"/"+version+suffix;
            try {
                var r=await VRCApi.Get<JObject>(endpoint,new Dictionary<string,string>{{"diagnosticCacheBust",DateTime.UtcNow.Ticks.ToString()}},forceRefresh:true);
                bool pending=(int?)r["error"]?["status_code"]==202||((string)r["error"]?["message"])=="Analysis not yet available";
                bool success=!pending&&(bool?)r["success"]==true;terminal&=!pending;passed&=success;
                results.Add(new JObject{["endpoint"]=endpoint,["passed"]=success,["pending"]=pending,["fileSize"]=r["fileSize"],["uncompressedSize"]=r["uncompressedSize"],["performanceRating"]=r["performanceRating"],["avatarStats"]=r["avatarStats"],["error"]=r["error"]});
            }
            catch(ApiErrorException e){passed=false;terminal&=(int)e.StatusCode!=202;results.Add(new JObject{["endpoint"]=endpoint,["passed"]=false,["statusCode"]=(int)e.StatusCode,["error"]=e.ErrorMessage});}
        }
        var report=new JObject{["success"]=passed&&terminal,["terminal"]=terminal,["avatarId"]=avatar.ID,["name"]=avatar.Name,["avatarVersion"]=avatar.Version,["pendingUpload"]=avatar.PendingUpload,["fileId"]=file,["fileVersion"]=version,["timeUtc"]=DateTime.UtcNow.ToString("O"),["results"]=results};
        Directory.CreateDirectory("Docs/MF4");File.WriteAllText("Docs/MF4/analysis.json",report.ToString());return report.ToString();
    }
}
