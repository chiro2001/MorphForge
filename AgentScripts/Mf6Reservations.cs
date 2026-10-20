using System;using System.IO;using System.Linq;using System.Collections.Generic;using System.Threading.Tasks;using Newtonsoft.Json.Linq;using VRC.SDKBase.Editor.Api;using MorphForge.Editor;
public static class Mf6Reservations {
 const string Owner="usr_c0dd064e-ac42-47a7-a17e-da6540382c55";
 const string Description="MorphForge unified 240-bit geometry; native 16K boxes, textured slopes and RGB polygon faces. Standalone MF6 test avatar.";
 public static async Task<string> Inspect(){
  var all=await VRCApi.Get<JArray>("avatars",new Dictionary<string,string>{{"user","me"},{"releaseStatus","private"},{"n","100"},{"offset","0"},{"sort","updated"},{"order","descending"}},forceRefresh:true);
  var candidates=new JArray();foreach(var item in all){
   if((string)item["name"]!="MorphForge MF6 16K"||(string)item["id"]==Mf6AvatarBuild.AvatarId)continue;
   bool empty=string.IsNullOrEmpty((string)item["assetUrl"])&&(!(item["unityPackages"] is JArray packages)||packages.Count==0);
   bool recent=DateTime.TryParse((string)item["created_at"],out var created)&&created.ToUniversalTime()>=DateTime.Parse("2026-10-20T07:20:00Z").ToUniversalTime();
   bool owned=(string)item["authorId"]==Owner&&(string)item["releaseStatus"]=="private";
   bool scoped=owned&&recent&&empty&&(bool?)item["pendingUpload"]==true&&(int?)item["version"]<=1&&(string)item["description"]==Description;
   candidates.Add(new JObject{["id"]=item["id"],["name"]=item["name"],["authorId"]=item["authorId"],["releaseStatus"]=item["releaseStatus"],["createdAt"]=item["created_at"],["pendingUpload"]=item["pendingUpload"],["version"]=item["version"],["empty"]=empty,["taskOwnedReservation"]=scoped});
  }
  var report=new JObject{["success"]=true,["retainedAvatarId"]=Mf6AvatarBuild.AvatarId,["candidates"]=candidates,["scope"]="Read-only scan for empty private MF6 reservations created by this task; uploaded or preexisting avatars are excluded."};File.WriteAllText("Docs/MF6/reservations.json",report.ToString());return report.ToString();
 }
 public static async Task<string> PruneOwnedEmpty(){
  var removed=new JArray();var kept=new JArray();
  // Exact IDs and creation times observed in this task's failed first uploads.
  foreach(string id in new[]{"avtr_3ba18a2a-828b-4f93-8ae1-bf64f36d6ac6","avtr_60097617-cce1-49a2-8637-77baf783bc78"}){
   if(id==Mf6AvatarBuild.AvatarId||id==Mf5AvatarBuild.AvatarId)throw new Exception("Cannot remove production identity");
   var item=await VRCApi.Get<JObject>("avatars/"+id,forceRefresh:true);
   bool empty=string.IsNullOrEmpty((string)item["assetUrl"])&&(!(item["unityPackages"] is JArray packages)||packages.Count==0);
   // JObject may hold a DateTime; serialize its ISO timezone instead of
   // converting through the machine's localized DateTime display string.
   string expected=id=="avtr_3ba18a2a-828b-4f93-8ae1-bf64f36d6ac6"?"2026-10-20T07:41:52.528Z":"2026-10-20T07:27:49.286Z";
   bool recent=DateTimeOffset.TryParse(item["created_at"]?.ToString(Newtonsoft.Json.Formatting.None).Trim('"'),out var created)&&created==DateTimeOffset.Parse(expected);
   // An unuploaded SDK reservation can omit its requested description. Exact
   // server identity + creation instant + no bundle + version1 scopes cleanup.
   bool scoped=(string)item["authorId"]==Owner&&(string)item["releaseStatus"]=="private"&&(string)item["name"]=="MorphForge MF6 16K"&&(int?)item["version"]==1&&empty&&recent;
   if(!scoped){kept.Add(new JObject{["id"]=id,["reason"]="Detailed identity/content guard did not match; left unchanged",["empty"]=empty,["recent"]=recent,["ownerMatches"]=(string)item["authorId"]==Owner,["nameMatches"]=(string)item["name"]=="MorphForge MF6 16K",["descriptionMatches"]=(string)item["description"]==Description,["version"]=item["version"],["createdAt"]=item["created_at"],["packageCount"]=(item["unityPackages"] as JArray)?.Count,["hasAssetUrl"]=!string.IsNullOrEmpty((string)item["assetUrl"])});continue;}
   // Keep the SDK cache response type consistent with the JObject inspection.
   await VRCApi.Delete<JObject>("avatars/"+id,forceRefresh:true);removed.Add(id);
  }
  var report=new JObject{["success"]=true,["retainedAvatarId"]=Mf6AvatarBuild.AvatarId,["removedEmptyTaskReservations"]=removed,["kept"]=kept};File.WriteAllText("Docs/MF6/reservation-cleanup.json",report.ToString());return report.ToString();
 }

}
