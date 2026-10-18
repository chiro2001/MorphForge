using System;
using System.Linq;
using System.Collections.Generic;
using System.Reflection;
using UnityEditor;
using UnityEditor.SceneManagement;
using UnityEngine;
using VRC.Dynamics;
using VRC.SDK3.Dynamics.Constraint.Components;
using VRC.SDK3.Dynamics.Contact.Components;

public static class PlacementInspect
{
    static object Describe(UnityEngine.Object value)
    {
        var properties=new List<object>();var so=new SerializedObject(value);var it=so.GetIterator();
        while(it.Next(true))properties.Add(new{it.propertyPath,kind=it.propertyType.ToString(),nativeType=it.type});
        return new{type=value.GetType().FullName,
            fields=value.GetType().GetFields(BindingFlags.Public|BindingFlags.Instance).Select(f=>new{f.Name,type=f.FieldType.FullName}).ToArray(),properties};
    }
    public static string Run()
    {
        var old=UnityEngine.SceneManagement.SceneManager.GetActiveScene();
        var temp=EditorSceneManager.NewScene(NewSceneSetup.EmptyScene,NewSceneMode.Additive);
        UnityEngine.SceneManagement.SceneManager.SetActiveScene(temp);
        try
        {
            var go=new GameObject("Inspect only");var pc=go.AddComponent<VRCParentConstraint>();
            pc.Sources.Add(new VRCConstraintSource{SourceTransform=go.transform,Weight=1});
            pc.Sources.Add(new VRCConstraintSource{SourceTransform=go.transform,Weight=0});
            var contact=go.AddComponent<VRCContactReceiver>();
            return Newtonsoft.Json.JsonConvert.SerializeObject(new{parent=Describe(pc),contact=Describe(contact),sourceFields=typeof(VRCConstraintSource).GetFields().Select(f=>new{f.Name,type=f.FieldType.FullName})},Newtonsoft.Json.Formatting.Indented);
        }
        finally{EditorSceneManager.CloseScene(temp,true);UnityEngine.SceneManagement.SceneManager.SetActiveScene(old);}
    }
}
