using System;
using System.Linq;
using System.Collections.Generic;
using UnityEditor;
using UnityEngine;
public static class PlacementBoundsInspect
{
 public static string Run(){var renderer=UnityEngine.Object.FindObjectsOfType<ParticleSystemRenderer>().First(x=>x.name=="Store");var so=new SerializedObject(renderer);var i=so.GetIterator();var entries=new List<string>();while(i.Next(true))if(i.propertyPath.ToLower().Contains("bound"))entries.Add(i.propertyPath+" "+i.propertyType);return Newtonsoft.Json.JsonConvert.SerializeObject(new{properties=entries,methods=typeof(ParticleSystemRenderer).GetMethods().Where(m=>m.Name.Contains("Bound")).Select(m=>m.ToString()).ToArray()});}
}
