using System;
using System.IO;
using System.Linq;
using UnityEngine;
using UnityEditor;
using UnityEngine.SceneManagement;
using BlackStartX.GestureManager;
using MorphForge.Editor;
using VRC.SDK3.Avatars.Components;
using Newtonsoft.Json.Linq;
public static class Mf4TestIsolation
{
    const string Proof="Docs/MF4/test-isolation.json";
    public static string Suspend()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        if(File.Exists(Proof)&&JObject.Parse(File.ReadAllText(Proof)).Value<bool>("suspended"))throw new Exception("Restore previous test isolation first.");
        var records=new JArray();
        var objects=Resources.FindObjectsOfTypeAll<GestureManager>().Select(g=>g.gameObject)
            .Concat(Resources.FindObjectsOfTypeAll<VRCAvatarDescriptor>().Select(d=>d.gameObject))
            .Where(go=>go.scene.IsValid()&&go.scene.isLoaded&&go.scene.path!=Mf4AvatarBuild.ScenePath).Distinct().ToArray();
        foreach(var go in objects)
        {
            records.Add(new JObject{["objectId"]=go.GetInstanceID(),["globalId"]=GlobalObjectId.GetGlobalObjectIdSlow(go).ToString(),["scene"]=go.scene.path,["sceneWasDirty"]=go.scene.isDirty,["active"]=go.activeSelf});go.SetActive(false);
        }
        Directory.CreateDirectory("Docs/MF4");var r=new JObject{["suspended"]=true,["records"]=records};File.WriteAllText(Proof,r.ToString());return r.ToString();
    }
    public static string Restore()
    {
        if(EditorApplication.isPlaying)throw new Exception("Edit Mode required.");
        var r=JObject.Parse(File.ReadAllText(Proof));
        foreach(var item in r["records"])
        {
            var go=EditorUtility.InstanceIDToObject((int)item["objectId"]) as GameObject;
            if(!go&&GlobalObjectId.TryParse((string)item["globalId"],out var id))go=GlobalObjectId.GlobalObjectIdentifierToObjectSlow(id) as GameObject;
            if(!go)throw new Exception("Cannot restore other scene test manager: "+item);
            go.SetActive((bool)item["active"]);
        }
        r["suspended"]=false;File.WriteAllText(Proof,r.ToString());return r.ToString();
    }
}
