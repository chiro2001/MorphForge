#!/usr/bin/env python3
"""Versioned real-model evaluation against an isolated loopback workbench.

The backend owns credentials. This runner never reads keys and does not enable
OSC. Cases may exercise message delivery while recording acknowledgements.
"""
from __future__ import annotations

import argparse
import collections
import hashlib
import ipaddress
import json
from pathlib import Path
import time
import urllib.parse
import urllib.request
import uuid


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--case', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--timeout', type=float, default=960)
    parser.add_argument('--require-cache-hit', action='store_true', help='Refuse paid evaluation until the current stable configuration has reported a real cache hit.')
    parser.add_argument('--lifetime-token-limit', type=int, default=0, help='Cap this data directory including prior probe usage; 0 leaves the case budget unchanged.')
    options = parser.parse_args()
    address = urllib.parse.urlsplit(options.url)
    if address.scheme != 'http' or address.hostname is None or address.port is None:
        parser.error('an explicit http loopback origin is required')
    if address.hostname != 'localhost' and not ipaddress.ip_address(address.hostname).is_loopback:
        parser.error('only loopback backends are supported')
    if address.port in (9000, 9012):
        parser.error('the target is an HTTP evaluation backend, not an OSC port')
    origin = options.url.rstrip('/')
    case_bytes = options.case.read_bytes()
    case = json.loads(case_bytes)
    options.output.mkdir(parents=True, exist_ok=True)

    def api(path: str, value=None):
        request = urllib.request.Request(origin + '/api/' + path)
        if value is not None:
            request.data = json.dumps(value, ensure_ascii=False).encode()
            request.add_header('Content-Type', 'application/json')
        with urllib.request.urlopen(request, timeout=90) as response:
            return json.load(response)

    initial = api('state')
    initial_scene = api('scene')['scene']
    if initial_scene['boxes'] and not case.get('continue', False):
        raise RuntimeError('the evaluation data directory must start with an empty scene')
    api('connect', {'enabled': False, 'auto': False, 'port': 9012})
    if case.get('enforceProtection'):
        api('project', {'protectedGroups': case['protectedGroups'],
                        'baseRevision': initial['revision']})
    settings = dict(case.get('settings', {}))
    if options.require_cache_hit:
        current = initial['harness']['settings']
        changed = [key for key in ('model','reasoning','language','preset','detailInstructions','sendImages') if key in settings and settings[key] != current.get(key)]
        monitor = initial['harness'].get('cacheMonitor', {})
        if changed or monitor.get('alert') or not (monitor.get('lastCachedTokens') or 0) > 0:
            raise RuntimeError('No paid evaluation: apply stable model/prompt settings and confirm an actual upstream cache hit first. Changed fields: '+','.join(changed))
    if options.lifetime_token_limit:
        spent = initial['harness']['usage']['total']['totalTokens']
        remaining = options.lifetime_token_limit - spent
        if remaining < 1000:
            raise RuntimeError('No paid evaluation: data-directory token limit is already exhausted')
        settings['maxRunTokens'] = min(settings.get('maxRunTokens', initial['harness']['settings']['maxRunTokens']), remaining)

    api('harness/config', settings)
    schema = api('harness/tools')
    versions = {'software': initial.get('software'), 'tools': schema.get('version'),
                'prompt': schema.get('promptVersion'), 'language': schema.get('language'),
                'schemaSha256': hashlib.sha256(json.dumps(schema, sort_keys=True,
                    ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()}
    def read_events(after):
        # The HTTP API caps each page at 500. Never advance using lastSeq:
        # it names the journal tail, which may be beyond the returned page.
        rows = []
        while True:
            page = api(f'events?after={after}')['events']
            rows.extend(page)
            if not page or len(page) < 500:
                return rows
            cursor = max(e['seq'] for e in page)
            if cursor <= after:
                raise RuntimeError('event pagination failed to advance')
            after = cursor

    before_events = read_events(0)
    first_seq = max((e['seq'] for e in before_events), default=0)
    events = []
    cursor = first_seq

    def refresh_events():
        nonlocal cursor
        page = read_events(cursor)
        events.extend(page)
        if page:
            cursor = max(e['seq'] for e in page)
        return events
    start = time.monotonic()
    if 'referenceIds' in case:
        api('harness/message', {'prompt': case['prompt'], 'referenceIds': case['referenceIds'],
                               'requestId': str(uuid.uuid4())})
        started = api('harness/state')
    else:
        started = api('harness/start', {'prompt': case['prompt']})
    initial_run = started['run']['id']
    actions = []
    seen_status = None
    terminal_since = None
    last_state = started
    report = {'case': case.get('id'), 'caseSha256': hashlib.sha256(case_bytes).hexdigest(),
              'backend': origin, 'initialRunId': initial_run, 'settings': settings, 'versions': versions,
              'actions': actions, 'success': False, 'requireCacheHit': options.require_cache_hit, 'lifetimeTokenLimit': options.lifetime_token_limit}
    try:
        while time.monotonic() - start < options.timeout:
            last_state = api('harness/state')
            refresh_events()
            if options.lifetime_token_limit and last_state['usage']['total']['totalTokens'] > options.lifetime_token_limit:
                api('harness/control', {'action': 'stop'})
                raise RuntimeError('evaluation data-directory token ceiling reached; stopped further requests')
            results = [e for e in events if e['kind'] == 'tool.result' and e.get('runId')]
            run = last_state.get('run') or {}
            phase = last_state.get('interaction', {}).get('phase', '')
            status = (run.get('id'), run.get('status'), run.get('steps'), phase)
            if status != seen_status:
                print(json.dumps({'run': status[0], 'status': status[1], 'step': status[2],
                                  'phase': phase, 'seconds': round(time.monotonic() - start, 1)}), flush=True)
                seen_status = status
            for number, action in enumerate(case.get('actions', [])):
                if any(a['number'] == number for a in actions):
                    continue
                when = action.get('when', {})
                if len(results) < when.get('toolResults', 0):
                    continue
                if time.monotonic() - start < when.get('seconds', 0):
                    continue
                if when.get('phase') and phase not in when['phase']:
                    continue
                if when.get('afterAction') is not None and not any(
                        a['number'] == when['afterAction'] for a in actions):
                    continue
                if when.get('afterAction') is not None:
                    previous = next(a for a in actions if a['number'] == when['afterAction'])
                    if time.monotonic() - start - previous['seconds'] < when.get('delay', 0):
                        continue
                if when.get('status') and run.get('status') not in when['status']:
                    continue
                if when.get('eventKind') and not any(e['kind'] == when['eventKind'] for e in events):
                    continue
                if when.get('awaitingProvider'):
                    request_seq = max((e['seq'] for e in events if e['kind'] == 'provider.request'), default=0)
                    response_seq = max((e['seq'] for e in events if e['kind'] in (
                        'provider.response', 'provider.incomplete', 'provider.cancelled')), default=0)
                    if request_seq <= response_seq:
                        continue
                request = dict(action['request'])
                if action['endpoint'] == 'harness/message':
                    request.setdefault('requestId', str(uuid.uuid4()))
                    request.setdefault('runId', run['id'])
                acknowledgement = api(action['endpoint'], request)
                row = {'number': number, 'seconds': time.monotonic() - start,
                       'request': request, 'ack': acknowledgement}
                if action.get('repeat', False):
                    replayed = api(action['endpoint'], request)
                    row['duplicateReplayed'] = replayed == acknowledgement
                actions.append(row)
                print(json.dumps({'action': number, 'accepted': True}), flush=True)
            active = run.get('status') in ('running', 'compacting', 'paused', 'interrupting')
            pending = last_state.get('interaction', {}).get('queue') or last_state.get('queued')
            if not active and not pending:
                terminal_since = terminal_since or time.monotonic()
                if time.monotonic() - terminal_since >= 3:
                    break
            else:
                terminal_since = None
            time.sleep(1)
        else:
            api('harness/control', {'action': 'stop'})
            raise TimeoutError('evaluation exceeded its wall-clock budget')
        state = api('state')
        scene = api('scene')['scene']
        refresh_events()
        model_events = [e for e in events if e.get('runId')]
        counts = collections.Counter(e['kind'] for e in model_events)
        tools = [e for e in model_events if e['kind'] == 'tool.result']
        errors = [e for e in tools if e['data'].get('result', {}).get('ok') is False]
        captures = [e for e in tools if e['data'].get('name') in ('capture_views','run_recipe','recipe_sources')
                    and bool(e['data'].get('result', {}).get('images'))]
        final = api('harness/tool', {'name': 'capture_views', 'arguments': {
            'revision': scene['revision'], 'views': ['hero', 'front', 'top']}})
        images = []
        for image in final['images']:
            path = image['url']
            if not path.startswith('/artifacts/'):
                raise RuntimeError('capture returned a non-artifact image path')
            with urllib.request.urlopen(origin + path, timeout=30) as response:
                blob = response.read(16 * 1024 * 1024)
            name = image['view'] + '.png'
            (options.output / name).write_bytes(blob)
            images.append({'view': image['view'], 'file': name,
                           'sha256': hashlib.sha256(blob).hexdigest()})
        checks = {
            'runCompleted': last_state['run']['status'] == 'completed',
            'allActionsExercised': len(actions) == len(case.get('actions', [])),
            'nonemptyScene': bool(scene['boxes']),
            'boxCapacity': len(scene['boxes']) <= case.get('maxBoxes', 16384),
            'modelUsedVisualFeedback': bool(captures),
            'modelCommitted': counts['revision.committed'] > 0,
            'modelSavedArtwork': counts['artwork.saved'] > 0,
            'oscRemainsDisabled': state['connection']['enabled'] is False,
        }
        for group in case.get('requiredGroups', []):
            checks[f'group:{group}'] = any(b.get('group') == group for b in scene['boxes'])
        for group in case.get('protectedGroups', []):
            def signature(box):
                return json.dumps(box, ensure_ascii=False, sort_keys=True)
            before = collections.Counter(signature(b) for b in initial_scene['boxes'] if b.get('group') == group)
            after = collections.Counter(signature(b) for b in scene['boxes'] if b.get('group') == group)
            checks[f'preserved:{group}'] = bool(before) and not (before - after)
        if case.get('requireCompact'):
            checks['contextCompacted'] = counts['compact.completed'] > 0
        if case.get('requireVisualIteration'):
            checks['geometryRevisedAfterVisualFeedback'] = any(
                first['seq'] < changed['seq'] < later['seq']
                and changed['revision'] > first['revision']
                and later['revision'] >= changed['revision']
                for first in captures for later in captures for changed in model_events
                if changed['kind'] in ('geometry.applied', 'geometry.replaced'))
        if case.get('targetVrc'):
            checks['vrcCoordinateRange'] = all(
                all(box['min'][axis] >= low and box['min'][axis] + box['size'][axis] <= high
                    for axis, low, high in ((0, -128, 128), (1, 0, 256), (2, -128, 128)))
                for box in scene['boxes'])
        if case.get('requireSavedViews'):
            captured = {image['artifact'] for event in captures
                        for image in event['data']['result']['images']}
            checks['savedArtworkHasActualViews'] = any(
                len(event['data'].get('views', [])) >= 3
                and event['data'].get('thumbnail') in captured
                and all(view in captured for view in event['data']['views'])
                for event in model_events if event['kind'] == 'artwork.saved')
        for row in actions:
            if row['request'].get('mode') in ('steer', 'queue', 'interrupt') and row['request'].get('prompt'):
                rid = row['request']['requestId']
                checks[f'messageApplied:{row["number"]}'] = any(
                    e['kind'] == 'harness.message.applied' and e['data'].get('requestId') == rid for e in events)
        report.update({'success': all(checks.values()), 'checks': checks,
                       'run': last_state['run'], 'interaction': last_state.get('interaction'),
                       'scene': {'revision': scene['revision'], 'boxes': len(scene['boxes']),
                                 'styles': len({b['styleId'] for b in scene['boxes']}),
                                 'groups': sorted({b.get('group', 'main') for b in scene['boxes']})},
                       'eventCounts': dict(counts), 'toolCalls': len(tools),
                       'usage': last_state.get('usage'),
                       'visualContext': [e['data'].get('visualContext') for e in model_events
                                         if e['kind'] == 'harness.request.prepared'],
                       'referenceImages': last_state.get('context', {}).get('referenceImages', []),
                       'toolNames': dict(collections.Counter(e['data']['name'] for e in tools)),
                       'toolErrors': [{'name': e['data']['name'], 'result': e['data']['result']}
                                      for e in errors],
                       'images': images, 'seconds': time.monotonic() - start})
        (options.output / 'scene.json').write_text(json.dumps(scene, ensure_ascii=False))
        (options.output / 'events.jsonl').write_text(''.join(
            json.dumps(event, ensure_ascii=False) + '\n' for event in events
        ))
        print(json.dumps({'success': report['success'], 'checks': checks,
                          'scene': report['scene'], 'seconds': round(report['seconds'], 1)}), flush=True)
    except Exception as error:
        report['error'] = str(error)
        report['seconds'] = time.monotonic() - start
        raise
    finally:
        (options.output / 'report.json').write_text(json.dumps(report, ensure_ascii=False, indent=2))
    if not report['success']:
        raise SystemExit(1)


if __name__ == '__main__':
    main()
