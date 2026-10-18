using System;
using System.Linq;
using UnityEditor;
using UnityEngine;
using BlackStartX.GestureManager;
using BlackStartX.GestureManager.Editor.Modules.Vrc3;

public static class ParticleStage1Live
{
    public static string Begin()
    {
        if(!EditorApplication.isPlaying)throw new InvalidOperationException("Enter Play Mode first.");
        var result=MorphForge.Editor.ParticleStage1Harness.StartOsc();
        // Use the saved scene camera and normal layers in both Game and Scene views.
        EditorApplication.ExecuteMenuItem("Window/General/Game");
        return result;
    }
}
