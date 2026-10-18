using System;

public static class CatalogEntry
{
    public static string Validate()
    {
        return MorphForge.Editor.CatalogMaterialBuild.Validate();
    }

    public static string Build()
    {
        return MorphForge.Editor.CatalogMaterialBuild.Build();
    }
}
