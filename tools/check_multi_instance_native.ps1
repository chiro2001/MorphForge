#requires -Version 7.0
param([Parameter(Mandatory)][string]$PackageDir,[Parameter(Mandatory)][string]$WorkRoot,[switch]$InteractiveChild)
$ErrorActionPreference='Stop';$ProgressPreference='SilentlyContinue'
if(-not $InteractiveChild){
 New-Item -ItemType Directory $WorkRoot -Force|Out-Null
 $task='MorphForge-MultiAudit-'+[guid]::NewGuid().ToString('N')
 $action=New-ScheduledTaskAction -Execute 'pwsh.exe' -Argument ('-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -File "'+$PSCommandPath+'" -InteractiveChild -PackageDir "'+$PackageDir+'" -WorkRoot "'+$WorkRoot+'"')
 $principal=New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
 Register-ScheduledTask -TaskName $task -Action $action -Principal $principal|Out-Null;Set-Content (Join-Path $WorkRoot 'task.txt') $task;Start-ScheduledTask $task;Write-Output $task;exit
}
$checks=[Collections.Generic.List[object]]::new();$owned=[Collections.Generic.List[object]]::new()
function Check($Name,$Value){$checks.Add(@{name=$Name;passed=[bool]$Value});if(-not $Value){throw $Name}}
function Api($Base,$Path,$Body=$null){if($null -eq $Body){return Invoke-RestMethod ($Base+'/api/'+$Path) -NoProxy};Invoke-RestMethod ($Base+'/api/'+$Path) -Method Post -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes(($Body|ConvertTo-Json -Depth 100))) -NoProxy}
function StartWindow([int]$Debug){
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$Debug --remote-allow-origins=*"
 $p=Start-Process (Join-Path $PackageDir 'morphforge-desktop.exe') -WorkingDirectory $PackageDir -PassThru;$owned.Add($p)
 $end=[DateTimeOffset]::UtcNow.AddSeconds(50);$base=$null;$state=$null
 while([DateTimeOffset]::UtcNow -lt $end){
  $cores=@(Get-CimInstance Win32_Process|Where-Object {$_.Name -eq 'morphforge.exe' -and $_.ParentProcessId -eq $p.Id})
  foreach($core in $cores){foreach($port in @(Get-NetTCPConnection -State Listen -OwningProcess $core.ProcessId -ErrorAction SilentlyContinue)){try{$base='http://127.0.0.1:'+$port.LocalPort;$state=Api $base 'state'}catch{}}}
  if($state){break};Start-Sleep -Milliseconds 200
 }
 if(-not $state){throw 'Instance did not initialize'}
 $p.Refresh();if($p.MainWindowHandle -ne [IntPtr]::Zero){[MorphForgeMultiAuditTitle]::SetWindowText($p.MainWindowHandle,('MorphForge 多窗口自动验收 '+$p.Id))|Out-Null}
 Api $base 'connect' @{enabled=$false;autoDetect=$false}|Out-Null
 return @{process=$p;base=$base;state=$state;debug=$Debug}
}
function CloseWindow($Window){
 $s=Api $Window.base 'state'
 Check 'closing own idle window only' ($s.project.id -eq $Window.state.project.id -and -not $s.connection.enabled -and $s.harness.interaction.phase -notin @('running','compacting','pausing','interrupting'))
 $p=$Window.process;$p.Refresh();if(-not $p.HasExited){if(-not $p.CloseMainWindow()){throw 'Native close failed'};if(-not $p.WaitForExit(20000)){throw 'Native window did not exit'}}
}
try{
 if($WorkRoot -notlike '*stream-v16*'){throw 'Isolated path required'}
 $env:MORPHFORGE_DATA=Join-Path $WorkRoot 'Data';$env:LOCALAPPDATA=Join-Path $WorkRoot 'localappdata';$env:APPDATA=Join-Path $WorkRoot 'roaming';$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $WorkRoot 'WebView2'
 $env:MORPHFORGE_API_BASE_URL='http://127.0.0.1:9';$env:MORPHFORGE_API_KEY='local-multi-window-fixture'
 foreach($dir in @($env:MORPHFORGE_DATA,$env:LOCALAPPDATA,$env:APPDATA,$env:WEBVIEW2_USER_DATA_FOLDER)){New-Item -ItemType Directory $dir -Force|Out-Null}
 Add-Type @'
using System;using System.Runtime.InteropServices;
public static class MorphForgeMultiAuditTitle{[DllImport("user32.dll",CharSet=CharSet.Unicode)]public static extern bool SetWindowText(IntPtr h,string t);}
'@
 $a=StartWindow 19233;$b=StartWindow 19234
 Check 'two real native windows run v16' ($a.state.software.version -eq '0.16.0' -and $b.state.software.version -eq '0.16.0')
 Check 'projects and conversations are independent' ($a.state.project.id -ne $b.state.project.id -and $a.state.harness.context.id -ne $b.state.harness.context.id)
 Check 'second instance uses a separate durable workspace' ($b.state.workspaceInstance.independent -eq $true)
 Api $a.base 'edit' @{op='box';min=@(0,0,0);size=@(4,4,4);styleId=1;baseRevision=0}|Out-Null
 $saved=Api $a.base 'artworks' @{action='save';title='多窗口共享库验收';tags=@('fixture')}
 $library=Api $b.base 'artworks'
 Check 'saved artwork immediately appears in the other library' (@($library.artworks|Where-Object {$_.id -eq $saved.artwork.id}).Count -eq 1)
 $before=(Api $b.base 'export').project
 Check 'library sharing did not change second geometry' ($before.scene.boxes.Count -eq 0)
 Api $b.base 'artworks' @{action='load';id=$saved.artwork.id;baseRevision=$before.scene.revision}|Out-Null
 Check 'second window can fork shared artwork' ((Api $b.base 'export').project.scene.boxes.Count -eq 1)
 CloseWindow $a
 Check 'closing first window leaves second core alive' ((Api $b.base 'state').project.id -eq $b.state.project.id)
 # Separate WebView2 profiles are essential: core survival alone would miss a
 # browser process killed by another native shell's Job Object.
 $targets=Invoke-RestMethod ('http://127.0.0.1:'+$b.debug+'/json/list') -NoProxy
 Check 'second WebView2 browser survives first Job close' (@($targets|Where-Object {$_.type -eq 'page' -and $_.url -like ($b.base+'*')}).Count -ge 1)
 $a2=StartWindow 19235
 Check 'reopening primary restores its project' ($a2.state.project.id -eq $a.state.project.id)
 CloseWindow $b
 $b2=StartWindow 19236
 Check 'reopening an idle independent slot restores project and context' ($b2.state.project.id -eq $b.state.project.id -and $b2.state.harness.context.id -eq $b.state.harness.context.id)
 Check 'independent slot retains its loaded geometry' ((Api $b2.base 'export').project.scene.boxes.Count -eq 1)
 CloseWindow $b2;CloseWindow $a2
 Check 'WebView cache slots are reused across reopening' (@(Get-ChildItem $env:WEBVIEW2_USER_DATA_FOLDER -Directory -Filter 'slot-*').Count -le 2)
 $report=@{success=$true;checks=$checks;realModelRequests=0;oscEnabled=$false;scope='Four real Windows Tauri launches, independent durable projects/contexts, shared artwork load, WebView2 browser survival, normal close. No main install or Avatar changes.'}
}catch{$report=@{success=$false;checks=$checks;error=$_.Exception.Message;position=$_.InvocationInfo.PositionMessage}}
finally{
 foreach($p in $owned){try{$p.Refresh();if(-not $p.HasExited){$p.CloseMainWindow()|Out-Null}}catch{}}
 $report|ConvertTo-Json -Depth 12|Set-Content (Join-Path $WorkRoot 'validation.json') -Encoding utf8NoBOM
}
