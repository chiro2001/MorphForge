#requires -Version 7.0
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ReportPath,[ValidateRange(2,100)][int]$Samples=25,[ValidateRange(50,2000)][int]$IntervalMs=100)
# Read-only diagnostics. Does not bind UDP, write parameters, change OSC config,
# advertise a service, select an Avatar or take over a sender channel.
$ErrorActionPreference='Stop'
$vrcIds=@(Get-Process -Name VRChat -ErrorAction SilentlyContinue|ForEach-Object {$_.Id})
if(-not $vrcIds.Count){throw 'VRChat is not running'}
$base=$null
foreach($listener in @(Get-NetTCPConnection -State Listen|Where-Object {$_.OwningProcess -in $vrcIds})){
 try{
  $candidate='http://127.0.0.1:'+$listener.LocalPort
  $info=Invoke-RestMethod ($candidate+'/?HOST_INFO') -TimeoutSec 2 -NoProxy
  if($info.EXTENSIONS.VALUE -and $info.OSC_TRANSPORT -eq 'UDP'){$base=$candidate;break}
 }catch{}
}
if(-not $base){throw 'No readable OSCQuery endpoint found on the VRChat process'}
$paths=@('MF6_Ctl','MF6_LastRx','MF6_Stage')+@(0..7|ForEach-Object {'MF6_P'+$_+'_DoneCount';'MF6_P'+$_+'_Busy';'MF6_P'+$_+'_Ready';'MF6_P'+$_+'_LastRx'})
$rows=[Collections.Generic.List[object]]::new()
$watch=[Diagnostics.Stopwatch]::StartNew()
for($i=0;$i -lt $Samples;$i++){
 $tree=Invoke-RestMethod ($base+'/avatar/parameters') -TimeoutSec 5 -NoProxy
 $values=[ordered]@{}
 foreach($name in $paths){$node=$tree.CONTENTS.$name;if($null -ne $node){$values[$name]=$node.VALUE}}
 if($values.Count -ne $paths.Count){throw 'Selected Avatar does not expose all expected MF6 diagnostics'}
 $rows.Add(@{elapsedMs=$watch.ElapsedMilliseconds;values=$values})
 if($i+1 -lt $Samples){Start-Sleep -Milliseconds $IntervalMs}
}
$first=$rows[0].values;$last=$rows[$rows.Count-1].values
$deltas=[ordered]@{};$total=0
foreach($i in 0..7){$delta=$last[('MF6_P'+$i+'_DoneCount')][0]-$first[('MF6_P'+$i+'_DoneCount')][0];$deltas['P'+$i]=$delta;$total+=$delta}
$signatures=@($rows|ForEach-Object {$_.values|ConvertTo-Json -Compress -Depth 4}|Select-Object -Unique)
$report=[ordered]@{success=$true;scope='Read-only real VRChat OSCQuery; no UDP sent or Avatar/configuration changed. Completion counters acknowledge local worker lifecycle only, not GPU output or other players.';samples=$rows.Count;distinctSnapshots=$signatures.Count;elapsedMs=$watch.ElapsedMilliseconds;summary=@{poolDoneDeltas=$deltas;totalCompletedWorkerOperations=$total};rows=$rows}
New-Item -ItemType Directory -Path (Split-Path -Parent ([IO.Path]::GetFullPath($ReportPath))) -Force|Out-Null
$report|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $ReportPath -Encoding utf8NoBOM
[ordered]@{success=$true;samples=$rows.Count;distinctSnapshots=$signatures.Count;completedWorkerOperations=$total;report=$ReportPath}|ConvertTo-Json -Compress
