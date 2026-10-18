using System;
using System.Linq;
using System.Reflection;
using VRC.SDK3.Dynamics.Constraint.Components;
public static class Stage4Types{
 public static string Run(){return Newtonsoft.Json.JsonConvert.SerializeObject(new[]{typeof(VRCPositionConstraint),typeof(VRCRotationConstraint),typeof(VRCAimConstraint)}.Select(t=>new{type=t.FullName,fields=t.GetFields(BindingFlags.Public|BindingFlags.Instance).Select(f=>new{f.Name,type=f.FieldType.FullName,values=f.FieldType.IsEnum?Enum.GetNames(f.FieldType):null})}),Newtonsoft.Json.Formatting.Indented);}
}
