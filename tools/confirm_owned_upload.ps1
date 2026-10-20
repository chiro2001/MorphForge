#requires -Version 7.0
param([Parameter(Mandatory)][int]$ProcessId,[Parameter(Mandatory)][string]$ExpectedContentId,[Parameter(Mandatory)][string]$UploadStatePath,[Parameter(Mandatory)][string]$ReportPath)
# This helper is restricted to the copyright dialog of this task's owned MF6
# avatar and is invoked only under the user's existing automatic-upload scope.
$ErrorActionPreference='Stop'
$state=Get-Content -LiteralPath $UploadStatePath -Raw|ConvertFrom-Json
if($state.targetAvatarId-ne $ExpectedContentId -or $state.status-ne 'running'){throw 'Owned MF6 upload identity/state mismatch'}
Add-Type -TypeDefinition @'
using System;using System.Text;using System.Collections.Generic;using System.Runtime.InteropServices;
public static class MFOwnedUploadDialog {
 public delegate bool Callback(IntPtr w,IntPtr p);
 [DllImport("user32.dll")]static extern bool EnumWindows(Callback c,IntPtr p);
 [DllImport("user32.dll")]static extern bool EnumChildWindows(IntPtr w,Callback c,IntPtr p);
 [DllImport("user32.dll")]static extern uint GetWindowThreadProcessId(IntPtr w,out uint id);
 [DllImport("user32.dll")]static extern bool IsWindowVisible(IntPtr w);
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern int GetWindowText(IntPtr w,StringBuilder s,int n);
 [DllImport("user32.dll")]public static extern bool PostMessage(IntPtr w,uint m,IntPtr a,IntPtr b);
 static string Text(IntPtr w){var s=new StringBuilder(2048);GetWindowText(w,s,s.Capacity);return s.ToString();}
 public static List<string> Titles(uint pid){var list=new List<string>();EnumWindows((w,p)=>{uint owner;GetWindowThreadProcessId(w,out owner);if(owner==pid&&IsWindowVisible(w))list.Add(Text(w));return true;},IntPtr.Zero);return list;}
 public static IntPtr Dialog(uint pid){IntPtr result=IntPtr.Zero;EnumWindows((w,p)=>{uint owner;GetWindowThreadProcessId(w,out owner);if(owner==pid&&IsWindowVisible(w)&&Text(w)=="Copyright ownership agreement")result=w;return true;},IntPtr.Zero);return result;}
 public static IntPtr Ok(IntPtr dialog){IntPtr result=IntPtr.Zero;EnumChildWindows(dialog,(w,p)=>{if(Text(w).Replace("&","")=="OK")result=w;return true;},IntPtr.Zero);return result;}
}
'@
$watch=[Diagnostics.Stopwatch]::StartNew();$dialog=[IntPtr]::Zero
while($watch.Elapsed.TotalSeconds-lt 15){$dialog=[MFOwnedUploadDialog]::Dialog($ProcessId);if($dialog-ne [IntPtr]::Zero){break};Start-Sleep -Milliseconds 200}
$report=[ordered]@{success=$false;contentId=$ExpectedContentId;scope='Standard SDK ownership dialog for the task-owned private MF6 upload';windows=[MFOwnedUploadDialog]::Titles($ProcessId)}
try{if($dialog-eq [IntPtr]::Zero){throw 'Expected SDK copyright dialog not found'};$ok=[MFOwnedUploadDialog]::Ok($dialog);if($ok-eq [IntPtr]::Zero){throw 'Expected OK button not found'};if(-not [MFOwnedUploadDialog]::PostMessage($ok,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)){throw 'Unable to invoke SDK ownership OK button'};$report.success=$true;$report.action='SDK dialog OK invoked under existing owned-content upload authorization'}finally{[IO.File]::WriteAllText($ReportPath,($report|ConvertTo-Json -Depth 10))}
