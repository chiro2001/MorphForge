#!/usr/bin/env python3
"""Reject known blocking Task waits in main-thread Pipeline entry scripts.

This focused source guard prevents the synchronous-wait regression that hung
AvatarDiagnosticPublish. It does not replace compiling and running the script.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
NON_CODE = re.compile(r'//[^\n]*|/\*[\s\S]*?\*/|@"(?:""|[^"])*"|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'')
BLOCKING = (
    (re.compile(r'\.\s*GetAwaiter\s*\(\s*\)\s*\.\s*GetResult\s*\('), 'sync Task.GetAwaiter().GetResult'),
    (re.compile(r'\bTask\s*\.\s*Wait(?:All|Any)\s*\('), 'sync Task.WaitAll/WaitAny'),
)


def violations(source: str):
    code = NON_CODE.sub(lambda match: '\n' * match.group().count('\n'), source)
    return [{'line': code.count('\n', 0, match.start()) + 1, 'reason': reason}
            for pattern, reason in BLOCKING for match in pattern.finditer(code)]


def self_test():
    assert violations('var account = CurrentAccount().GetAwaiter().GetResult();')
    assert violations('work.GetAwaiter ( )\n .GetResult ( );')
    assert violations('Task.WaitAll(tasks);')
    assert not violations('public static async Task<string> StartUpload() { var a = await CurrentAccount(); }')
    assert not violations('// x.GetAwaiter().GetResult();\nvar s = "Task.WaitAll(tasks)";')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('files', nargs='*', type=Path)
    args = parser.parse_args()
    self_test()
    paths = args.files or sorted((ROOT / 'AgentScripts').glob('*.cs'))
    failures = [{'path': str(path.relative_to(ROOT) if path.is_relative_to(ROOT) else path), **v}
                for path in paths for v in violations(path.read_text(encoding='utf-8-sig'))]
    print(json.dumps({'success': not failures, 'selfTest': True,
                      'filesScanned': len(paths), 'violations': failures}))
    raise SystemExit(bool(failures))


if __name__ == '__main__':
    main()
