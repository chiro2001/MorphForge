#!/usr/bin/env python3
"""Copy the checked publisher kit to the workstation without touching Unity Assets."""
import hashlib, json, subprocess
from pathlib import Path
from unity_remote import DEFAULT_HOST, run_pwsh

ROOT=Path(__file__).resolve().parents[1]

def main():
    root=ROOT/'Releases/MorphForge-0.27.0'
    package=root/'MorphForge-0.27.0-PublisherKit.zip'
    digest=hashlib.sha256(package.read_bytes()).hexdigest()
    report=json.loads((ROOT/'docs/releases/0.27.0/verification.json').read_text())
    assert report['success']and digest==report['publisherKitSha256'],'Run check_booth_publish.py --seal first'
    result=run_pwsh("New-Item -ItemType Directory -Path E:/tmp/morphforge/booth-20261025 -Force | Out-Null",capture_output=True)
    if result.returncode:raise RuntimeError(result.stderr.decode('utf-8-sig'))
    remote='E:/tmp/morphforge/booth-20261025/'+package.name
    subprocess.run(['scp','-q',str(package),DEFAULT_HOST+':'+remote],check=True)
    script=r'''
$release='D:\Projects\VRChatProjects2\MorphForge\Releases\MorphForge-0.27.0'
$upload='E:\tmp\morphforge\booth-20261025\MorphForge-0.27.0-PublisherKit.zip'
$expected='EXPECTED_DIGEST'
if((Get-FileHash -LiteralPath $upload -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected){throw 'Uploaded publisher kit digest differs'}
$destination=Join-Path $release 'BOOTH-Publish'
if(Test-Path -LiteralPath $destination){
 $marker=Join-Path $destination 'publish-preparation.json'
 if(!(Test-Path -LiteralPath $marker)){throw 'Unrecognized existing destination'}
 $old=Get-Content -Raw -LiteralPath $marker | ConvertFrom-Json
 if($old.generatedBy -ne 'tools/build_booth_publish.py' -or $old.desktopVersion -ne '0.27.0'){throw 'Destination belongs to another release'}
}
New-Item -ItemType Directory -Path $release -Force | Out-Null
$stage=Join-Path $release ('.booth-staging-'+[guid]::NewGuid().ToString('N'))
Expand-Archive -LiteralPath $upload -DestinationPath $stage
$manifest=Get-Content -Raw -LiteralPath (Join-Path $stage 'publisher-files-sha256.json') | ConvertFrom-Json
$count=0
foreach($property in $manifest.PSObject.Properties){
 $file=Join-Path $stage $property.Name
 if((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $property.Value){throw ('File digest differs: '+$property.Name)}
 $count++
}
if(@(Get-ChildItem -LiteralPath $stage -Recurse -File).Count -ne $count+1){throw 'Unexpected file count after extraction'}
if(Test-Path -LiteralPath $destination){Move-Item -LiteralPath $destination -Destination ($destination+'.backup-'+[guid]::NewGuid().ToString('N'))}
Move-Item -LiteralPath $stage -Destination $destination
Copy-Item -LiteralPath $upload -Destination (Join-Path $release 'MorphForge-0.27.0-PublisherKit.zip')
[IO.File]::WriteAllText((Join-Path $release 'MorphForge-0.27.0-PublisherKit.zip.sha256'),$expected+'  MorphForge-0.27.0-PublisherKit.zip'+[Environment]::NewLine,[Text.UTF8Encoding]::new($false))
$preparation=Get-Content -Raw -LiteralPath (Join-Path $destination 'publish-preparation.json') | ConvertFrom-Json
[pscustomobject]@{success=$true;destination=$destination;entryPoint=(Join-Path $destination 'index.html');publisherKitSha256=$expected;verifiedFiles=$count;fileCount=$count+1;buyerZipSha256=$preparation.zipSha256;assetsOrInstalledProgramModified=$false;publishedToBooth=$false} | ConvertTo-Json
'''.replace('EXPECTED_DIGEST',digest)
    result=run_pwsh(script,capture_output=True)
    if result.returncode:raise RuntimeError(result.stderr.decode('utf-8-sig'))
    record=json.loads(result.stdout.decode('utf-8-sig'))
    assert record['success']and record['publisherKitSha256']==digest
    (ROOT/'docs/releases/0.27.0/windows-copy.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(record,ensure_ascii=False))

if __name__=='__main__':main()
