using UnityEditor;
public static class PluginImport {
 public static string Run(){AssetDatabase.ImportPackage("E:/tmp/morphforge/avatar-plugin/dist/MorphForgeAvatar-1.0.0.unitypackage",false);return "Unity package imported; poll compilation before acceptance";}
}
