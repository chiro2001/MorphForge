#!/usr/bin/env python3
"""Run placement tests against the current Stage3 Play Mode without touching other scenes."""
import argparse,json
from pathlib import Path
from check_stage2 import pipeline
ROOT=Path(__file__).resolve().parents[1]
def entry(file,name):
    r=pipeline('run_script','--file','AgentScripts/'+file+'.cs','--entry',file+'.'+name,'--timeout_ms','60000')
    if not r.get('success'):raise RuntimeError(r)
    return json.loads(r['result'])
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--reuse-owned-play',action='store_true');args=parser.parse_args()
    status=pipeline('editor_status')
    if status['playMode']!='stopped' and not args.reuse_owned_play:raise RuntimeError('Refusing to take over existing Play Mode; --reuse-owned-play is only for a test session you started')
    if status['playMode']=='stopped':
        pipeline('editor_play')
        # Play entry can trigger a domain reload; wait for explicit ready.
        import time
        for _ in range(30):
            try:
                s=pipeline('editor_status')
                if not s['compiling'] and not s['domainReloadInProgress']:break
            except Exception:pass
            time.sleep(.3)
    entry('PlacementEntry','StartOsc')
    r=entry('PlacementCheck','Check');out=ROOT/'docs/stage3';out.mkdir(parents=True,exist_ok=True)
    (out/'placement-validation.json').write_text(json.dumps(r,ensure_ascii=False,indent=2))
    for c in r['checks']:print(('PASS ' if c['pass'] else 'FAIL ')+c['name'])
    if not r['success']:raise SystemExit(1)
if __name__=='__main__':main()
