#!/usr/bin/env python3
"""Loopback-only transparent API relay with payload-prefix diagnostics.

Forwards request and response bodies byte for byte, including SSE. Credentials
are forwarded but never logged. Reports contain hashes/counts, not prompt or
image contents. Optional local HF tokenizer files inspect text Chat templates;
multimodal expansion and an actual server's KV-cache are outside that check.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import threading
import urllib.error
import urllib.parse
import urllib.request


def digest(value):
    return hashlib.sha256(json.dumps(value, ensure_ascii=False, sort_keys=True,
                                    separators=(',', ':')).encode()).hexdigest()


def common_prefix(a, b):
    for index, (left, right) in enumerate(zip(a, b)):
        if left != right:
            return index
    return min(len(a), len(b))


class TemplateProbe:
    def __init__(self, directory):
        from jinja2.sandbox import ImmutableSandboxedEnvironment
        from tokenizers import Tokenizer
        config = json.loads((directory / 'tokenizer_config.json').read_text())
        environment = ImmutableSandboxedEnvironment(trim_blocks=True, lstrip_blocks=True)
        environment.filters['tojson'] = lambda value, **kwargs: json.dumps(value, ensure_ascii=False, **kwargs)
        def raise_exception(message):
            raise ValueError(message)
        environment.globals['raise_exception'] = raise_exception
        self.template = environment.from_string(config['chat_template'])
        self.tokenizer = Tokenizer.from_file(str(directory / 'tokenizer.json'))
        self.hashes = {name: hashlib.sha256((directory / name).read_bytes()).hexdigest()
                       for name in ('tokenizer_config.json', 'tokenizer.json')}

    def render(self, body):
        if not isinstance(body.get('messages'), list):
            return None
        # A text tokenizer cannot validate a multimodal processor. Keep that
        # distinction explicit instead of silently flattening image messages.
        if any(isinstance(message.get('content'), list) for message in body['messages']):
            return None
        prompt = self.template.render(messages=body['messages'], tools=body.get('tools'),
                                      add_generation_prompt=True)
        return prompt, self.tokenizer.encode(prompt, add_special_tokens=False).ids


class Recorder:
    def __init__(self, tokenizer_dir=None):
        self.lock = threading.Lock()
        self.rows = []
        self.previous = {}
        self.template = TemplateProbe(tokenizer_dir) if tokenizer_dir else None

    def record(self, path, raw):
        try:
            body = json.loads(raw)
        except (ValueError, UnicodeDecodeError):
            return
        field = 'messages' if 'messages' in body else 'input'
        messages = body.get(field)
        if not isinstance(messages, list):
            return
        static = {key: body.get(key) for key in ('model', 'tools', 'instructions',
                  'reasoning', 'reasoning_effort', 'chat_template_kwargs', 'tool_choice')}
        leading = []
        for message in messages:
            if message.get('role') not in ('system', 'developer'):
                break
            leading.append(message)
        static['leadingInstructions'] = leading
        signature = digest(static)
        hashes = [digest(message) for message in messages]
        rendered = self.template.render(body) if self.template else None
        key = path
        with self.lock:
            previous = self.previous.get(key)
            row = {'number': len(self.rows) + 1, 'path': path,
                   'requestSha256': hashlib.sha256(raw).hexdigest(),
                   'requestBytes': len(raw), 'staticSha256': signature,
                   'messages': len(messages), 'images': raw.count(b'data:image/'),
                   'comparison': None}
            if previous:
                shared = common_prefix(previous['messages'], hashes)
                same_static = signature == previous['static']
                row['comparison'] = {'appendOnly': same_static and shared == len(previous['messages']),
                    'staticPrefixUnchanged': same_static,
                    'sharedMessages': shared, 'previousMessages': len(previous['messages']),
                    'firstChangedMessage': shared if shared < len(previous['messages']) else None}
                if rendered and previous['rendered']:
                    old_text, old_tokens = previous['rendered']
                    text, tokens = rendered
                    row['template'] = {'scope': 'local_text_chat_template',
                        'appendOnlyText': text.startswith(old_text),
                        'previousCharacters': len(old_text), 'commonCharacters': common_prefix(old_text, text),
                        'previousTokens': len(old_tokens), 'commonTokens': common_prefix(old_tokens, tokens),
                        'currentTokens': len(tokens),
                        'tokenPrefixUnchanged': tokens[:len(old_tokens)] == old_tokens}
            self.rows.append(row)
            self.previous[key] = {'messages': hashes, 'rendered': rendered, 'static': signature}
            return row

    def report(self):
        with self.lock:
            return {'scope': 'transparent HTTP relay; no claim of server KV-cache hit',
                    'templateFiles': self.template.hashes if self.template else None,
                    'requests': list(self.rows)}


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def make_server(upstream, recorder, port=0):
    parsed = urllib.parse.urlsplit(upstream)
    if parsed.scheme not in ('http', 'https') or not parsed.netloc or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError('upstream must be an HTTP(S) base URL without embedded credentials')
    opener = urllib.request.build_opener(NoRedirect())
    hop_headers = {'connection', 'keep-alive', 'proxy-authenticate', 'proxy-authorization',
                   'te', 'trailer', 'transfer-encoding', 'upgrade', 'host', 'content-length'}

    class Relay(BaseHTTPRequestHandler):
        protocol_version = 'HTTP/1.1'
        def log_message(self, *_args):
            pass
        def do_GET(self):
            if self.path == '/__prefix__/report':
                data = json.dumps(recorder.report()).encode()
                self.send_response(200); self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(data))); self.end_headers(); self.wfile.write(data)
            else:
                self.send_error(404)
        def do_POST(self):
            if self.path not in ('/chat/completions', '/responses', '/v1/chat/completions', '/v1/responses'):
                self.send_error(404); return
            length = int(self.headers.get('Content-Length', '0'))
            if not 0 < length <= 128 * 1024 * 1024:
                self.send_error(413); return
            raw = self.rfile.read(length)
            row = None
            # Observation failures must never transform or block valid traffic.
            try:
                row = recorder.record(self.path, raw)
            except Exception as error:
                with recorder.lock:
                    recorder.rows.append({'path': self.path, 'probeError': type(error).__name__})
            request = urllib.request.Request(upstream.rstrip('/') + self.path, data=raw,
                headers={key: value for key, value in self.headers.items() if key.lower() not in hop_headers})
            try:
                try:
                    response = opener.open(request, timeout=240)
                except urllib.error.HTTPError as error:
                    response = error
                with response:
                    self.send_response(response.status)
                    for key, value in response.headers.items():
                        if key.lower() not in hop_headers:
                            self.send_header(key, value)
                    self.send_header('Connection', 'close'); self.end_headers()
                    response_hash = hashlib.sha256()
                    response_bytes = 0
                    while chunk := response.read1(65536):
                        response_hash.update(chunk); response_bytes += len(chunk)
                        self.wfile.write(chunk); self.wfile.flush()
                    if row is not None:
                        with recorder.lock:
                            row.update({'responseSha256':response_hash.hexdigest(),
                                        'responseBytes':response_bytes,'status':response.status})
            except (BrokenPipeError, ConnectionResetError):
                pass
            except urllib.error.URLError:
                self.send_error(502, 'upstream unavailable')
            finally:
                self.close_connection = True

    return ThreadingHTTPServer(('127.0.0.1', port), Relay)


@contextmanager
def running_proxy(upstream, tokenizer_dir=None):
    recorder = Recorder(tokenizer_dir)
    server = make_server(upstream, recorder)
    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    try:
        yield f'http://127.0.0.1:{server.server_port}', recorder
    finally:
        server.shutdown(); server.server_close(); thread.join()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', required=True)
    parser.add_argument('--port', type=int, default=18790)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--tokenizer-dir', type=Path)
    args = parser.parse_args()
    recorder = Recorder(args.tokenizer_dir)
    server = make_server(args.upstream, recorder, args.port)
    print(f'Transparent relay ready: http://127.0.0.1:{server.server_port}', flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close(); args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(recorder.report(), ensure_ascii=False, indent=2) + '\n')


if __name__ == '__main__':
    main()
