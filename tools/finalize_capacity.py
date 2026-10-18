#!/usr/bin/env python3
"""Inspect real 16k UDP run, preserve failures, repair and check pool recycling."""
import json
from pathlib import Path
from check_stage2 import pipeline, remote_file
from unity_remote import DEFAULT_PROJECT, pwsh_literal, run_pwsh

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'docs/stage4'


def entry(name):
    value = pipeline('run_script', '--file', 'AgentScripts/Stage4Capacity.cs',
                     '--entry', 'Stage4Capacity.' + name, '--timeout_ms', '60000')
    return json.loads(value['result'])


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    report = {'success': False, 'port': 9012, 'repairInterval': .5,
              'scope': 'Actual Windows UDP -> native GM/FX/particles; no prefill; no remote ACK',
              'captures': [], 'senders': [], 'checks': []}
    serial = 0

    def check(name, passed):
        report['checks'].append({'name': name, 'pass': bool(passed)})
        print(('PASS ' if passed else 'FAIL ') + name, flush=True)
        if not passed:
            raise AssertionError(name)

    def capture(label):
        value = entry('CompleteReport')
        value['label'] = label
        snapshot = f'Docs/Stage4/capacity-{label}-particles.bin'
        saved = run_pwsh('Set-Location -LiteralPath ' + pwsh_literal(DEFAULT_PROJECT)
                         + '; Copy-Item -LiteralPath ' + pwsh_literal(value['nativeSnapshot'])
                         + ' -Destination ' + pwsh_literal(snapshot) + ' -Force', capture_output=True)
        if saved.returncode:
            raise RuntimeError(saved.stderr.decode(errors='replace'))
        value['nativeSnapshot'] = snapshot
        report['captures'].append(value)
        (OUT / ('capacity-' + label + '.json')).write_text(json.dumps(value, indent=2))
        print(json.dumps({k: value[k] for k in ('label', 'count', 'unique', 'malformed',
                                               'metadataMismatch', 'received', 'completed')}), flush=True)
        return value

    def send(commands, previous):
        nonlocal serial
        serial += 1
        path = f'Docs/Stage4/capacity-repair-{serial}.json'
        remote_file(path, commands)
        args = ['tools/send_mf3.py', '--input', path, '--port', '9012', '--seq',
                str((previous['lastRxSeq'] + 1) % 16), '--interval', '.5']
        result = run_pwsh('Set-Location -LiteralPath ' + pwsh_literal(DEFAULT_PROJECT)
                          + '; & python ' + ' '.join(pwsh_literal(a) for a in args)
                          + '; exit $LASTEXITCODE', capture_output=True)
        if result.returncode:
            raise RuntimeError(result.stderr.decode(errors='replace'))
        report['senders'].append(json.loads(result.stdout.decode('utf-8-sig')))

    def voxel(index, op='voxel'):
        return {'op': op, 'min': [80 + index % 32 * 3, index // 1024 * 3,
                                 80 + index // 32 % 32 * 3]}

    try:
        value = capture('first-pass')
        check('long sender finished before repair', value['progress']['finished'])
        report['firstPassLossless'] = value['success']
        for attempt in range(1, 5):
            if value['success']:
                break
            check('only missing particles may be repaired', value['malformed'] == 0
                  and value['metadataMismatch'] == 0 and value['count'] == value['unique'])
            send([{'op': 'brush', 'styleId': 1}] + [voxel(i) for i in value['missing']], value)
            value = capture('repair-' + str(attempt))
        check('16384 unique native boxes after real UDP', value['success'])
        report['fullCapacityFrameSample'] = entry('FrameSample')
        send([voxel(i, 'remove') for i in range(16)], value)
        value = capture('remove-16')
        check('full pool removes exactly 16 boxes', value['count'] == 16368
              and set(value['missing']) == set(range(16)))
        send([voxel(i) for i in range(16)], value)
        value = capture('refill-16')
        check('freed slots accept replacement boxes', value['success'])
        send([{'op': 'erase', 'min': [80, 0, 80], 'size': [10, 10, 10]}], value)
        value = capture('region-erase')
        expected_missing = {x + z * 32 + y * 1024 for x in range(4)
                            for y in range(4) for z in range(4)}
        check('region erase removes 64 exact centers', value['count'] == 16320
              and set(value['missing']) == expected_missing)
        send([{'op': 'clear'}], value)
        value = capture('clear')
        check('clear empties native store', value['count'] == 0 and not value['busy']
              and not value['inbox'])
        report['success'] = True
    except Exception as exc:
        report['error'] = str(exc)
        raise
    finally:
        (OUT / 'capacity-validation.json').write_text(json.dumps(report, indent=2))


if __name__ == '__main__':
    main()
