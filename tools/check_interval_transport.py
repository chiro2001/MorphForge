#!/usr/bin/env python3
"""Verify seven send intervals using real loopback UDP, never port 9000.

Use an empty, disposable backend. This measures the external sender only; it
does not measure Avatar execution or remote-player delivery.
"""
import argparse
import json
import socket
import statistics
import threading
import time
import urllib.request
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--url', required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    checks, rows, timestamps = [], [], []
    stop = threading.Event()
    receiver = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    receiver.bind(('127.0.0.1', 0))
    receiver.settimeout(.1)
    port = receiver.getsockname()[1]
    if port in (9000, 9001, 9012):
        raise RuntimeError('Refusing a non-fixture receiver port')

    def api(path, body=None):
        request = urllib.request.Request(a.url + '/api/' + path,
            data=None if body is None else json.dumps(body).encode(),
            headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(request, timeout=30) as r:
            return json.load(r)

    def check(name, ok):
        checks.append({'name': name, 'passed': bool(ok)})
        print(('PASS ' if ok else 'FAIL ') + name, flush=True)
        if not ok:
            raise AssertionError(name)

    def read():
        while not stop.is_set():
            try:
                packet, _ = receiver.recvfrom(65535)
                if not packet.startswith(b'#bundle\0'):
                    raise AssertionError('Expected real OSC bundles')
                timestamps.append(time.monotonic())
            except socket.timeout:
                pass

    thread = threading.Thread(target=read, daemon=True)
    report = {'success': False, 'checks': checks, 'measurements': rows,
              'scope': 'Actual Rust sender and loopback UDP. No native/remote ACK or multiplayer reliability claim.',
              'realModelRequests': 0, 'fixturePort': port}
    enabled = False
    try:
        initial = api('state')
        check('empty isolated workspace and OSC disabled', initial['revision'] == 0 and not initial['connection']['enabled'])
        api('vrc/profile', {'action': 'bundled_mf6'})
        api('harness/tool', {'name': 'run_recipe', 'arguments': {
            'baseRevision': 0, 'mode': 'replace',
            'code': "for(let i=0;i<48;i++)mf.box([i*2-48,0,0],[1,1,1],{style:1});"}})
        thread.start()
        api('connect', {'enabled': True, 'port': port, 'profile': 'mf6', 'interval': .1,
                        'loopReplay': True, 'refreshSeconds': 0})
        enabled = True
        for interval in [.1, .2, .25, .28, .3, .4, .5]:
            before = api('state')['connection']
            api('connect', {'interval': interval})
            start = len(timestamps)
            deadline = time.monotonic() + 12
            while len(timestamps) < start + 8 and time.monotonic() < deadline:
                time.sleep(.02)
            check(f'{interval:.2f}s continues emitting real bundles', len(timestamps) >= start + 8)
            # Discard two packets to exclude the pending wait under the old rate.
            sample = timestamps[start + 2:start + 8]
            gaps = [b - x for x, b in zip(sample, sample[1:])]
            row = {'configuredSeconds': interval, 'observedSeconds': gaps,
                   'minimumSeconds': min(gaps), 'medianSeconds': statistics.median(gaps)}
            rows.append(row)
            check(f'{interval:.2f}s pacing matches configuration',
                  min(gaps) >= interval - .01 and abs(statistics.median(gaps) - interval) < .08)
            after = api('state')['connection']
            check(f'{interval:.2f}s rate change preserves the target',
                  after['desiredRevision'] == before['desiredRevision'] and after['port'] == port and after['enabled'])
        report['success'] = True
    except Exception as e:
        report['error'] = str(e)
        raise
    finally:
        if enabled:
            api('connect', {'enabled': False})
        stop.set()
        if thread.is_alive():
            thread.join(timeout=1)
        receiver.close()
        (a.out / 'validation.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
