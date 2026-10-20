#requires -Version 7.0
param([Parameter(Mandatory)][string]$PackageDir,[Parameter(Mandatory)][string]$WorkRoot,[int]$DebugPort=19220,[switch]$InteractiveChild)
$ErrorActionPreference='Stop'
if(-not $InteractiveChild){
 New-Item -ItemType Directory -Path $WorkRoot -Force|Out-Null
 $task='MorphForge-GuiAudit-'+[guid]::NewGuid().ToString('N')
 $arguments='-NoLogo -NoProfile -NonInteractive -WindowStyle Hidden -File "'+$PSCommandPath+'" -InteractiveChild -PackageDir "'+$PackageDir+'" -WorkRoot "'+$WorkRoot+'" -DebugPort '+$DebugPort
 $action=New-ScheduledTaskAction -Execute 'pwsh.exe' -Argument $arguments
 $principal=New-ScheduledTaskPrincipal -UserId ([Security.Principal.WindowsIdentity]::GetCurrent().Name) -LogonType Interactive -RunLevel Limited
 Register-ScheduledTask -TaskName $task -Action $action -Principal $principal|Out-Null
 Set-Content (Join-Path $WorkRoot 'native-task.txt') $task
 Start-ScheduledTask -TaskName $task
 Write-Output "Started isolated interactive GUI task: $task"
 exit
}
try {
 $profile=Join-Path $WorkRoot 'profile-interactive'
 foreach($dir in @($profile,(Join-Path $profile 'localappdata'),(Join-Path $profile 'roaming'),(Join-Path $profile 'WebView2'),(Join-Path $WorkRoot 'downloads'))){New-Item -ItemType Directory -Path $dir -Force|Out-Null}
 $env:MORPHFORGE_DATA=Join-Path $WorkRoot 'interactive-data'
 $env:LOCALAPPDATA=Join-Path $profile 'localappdata';$env:APPDATA=Join-Path $profile 'roaming';$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $profile 'WebView2'
 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$DebugPort --remote-allow-origins=*"
 $env:MORPHFORGE_API_BASE_URL='http://127.0.0.1:9';$env:MORPHFORGE_API_KEY='local-gui-validation-only'
 $p=Start-Process -FilePath (Join-Path $PackageDir 'morphforge-desktop.exe') -WorkingDirectory $PackageDir -PassThru
 @{pid=$p.Id;package=$PackageDir;data=$env:MORPHFORGE_DATA;downloads=(Join-Path $WorkRoot 'downloads');profile=$profile}|ConvertTo-Json|Set-Content (Join-Path $WorkRoot 'native-session.json') -Encoding utf8
} catch { $_|Out-String|Set-Content (Join-Path $WorkRoot 'launch-error.txt');throw }
