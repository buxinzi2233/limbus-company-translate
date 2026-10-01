"""Exercise real LCT scope import, HTTP pagination, MCP and source retirement in an isolated database."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def fingerprint(path: Path) -> str:
    digest = hashlib.sha256()
    with sqlite3.connect(path.resolve().as_uri() + '?mode=ro', uri=True) as conn:
        for table in ('segments', 'translation_events', 'tm_entries'):
            for row in conn.execute(f'SELECT * FROM {table} ORDER BY id'):
                digest.update(json.dumps(row, ensure_ascii=False).encode())
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--database', type=Path, required=True)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--project', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    spec = importlib.util.spec_from_file_location('reference_checks', Path(__file__).with_name('verify-references.py'))
    assert spec and spec.loader
    helpers = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helpers)
    before = fingerprint(args.database)
    manifest = json.loads(args.manifest.read_text())
    expected = {row['segment_id'] for row in manifest['segments']}
    with tempfile.TemporaryDirectory(prefix='lct-task-scope-') as folder:
        root = Path(folder)
        with sqlite3.connect(args.database.resolve().as_uri() + '?mode=ro', uri=True) as source:
            with sqlite3.connect(root / 'project.db') as target:
                source.backup(target)
            source_dir, = source.execute('SELECT live_client_kr_dir FROM projects WHERE id=?', (args.project,)).fetchone()
            inventory_count, = source.execute('SELECT COUNT(*) FROM segments WHERE project_id=? AND active=1', (args.project,)).fetchone()
            excluded_file, = source.execute("SELECT id FROM files WHERE project_id=? AND relative_path='StoryData/S1004B.json'", (args.project,)).fetchone()
        baseline = fingerprint(root / 'project.db')
        command = [str(args.binary.resolve()), '--data-dir', str(root)]
        importer = command + ['scope-import', '--project-id', args.project, '--name', 'Part2 scope integration', '--input']
        first = subprocess.run(importer + [str(args.manifest.resolve())], capture_output=True, text=True, check=True)
        result = json.loads(first.stdout)
        assert result['total_segments'] == len(expected) and result['total_files'] == len(manifest['files'])
        subprocess.run(importer + [str(args.manifest.resolve())], capture_output=True, text=True, check=True)
        assert fingerprint(root / 'project.db') == baseline
        rejected = []
        for case in ('project', 'hash', 'identity', 'duplicate', 'count', 'pointer'):
            bad = json.loads(args.manifest.read_text())
            if case == 'project':
                bad['project_id'] = 'not-the-bound-project'
            elif case == 'hash':
                bad['files'][0]['sha256'] = '0' * 64
            elif case == 'identity':
                bad['segments'][-1]['source_identity'] += 'changed'
            elif case == 'duplicate':
                bad['segments'].append(bad['segments'][0])
            elif case == 'count':
                bad['files'][0]['selected_segments'] += 1
            elif case == 'pointer':
                bad['segments'][0]['pointer'] = '/not-a-source-field'
            path = root / f'{case}.json'
            path.write_text(json.dumps(bad, ensure_ascii=False))
            attempt = subprocess.run(importer + [str(path)], capture_output=True, text=True)
            assert attempt.returncode != 0 and attempt.stderr, case
            rejected.append({'case': case, 'error': attempt.stderr.strip()[-700:]})
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        base = f'http://127.0.0.1:{port}'
        with tempfile.TemporaryFile(mode='w+') as log:
            server = subprocess.Popen(command + ['ui', '--project-id', args.project, '--port', str(port), '--no-open'], stdout=log, stderr=log)
            try:
                for attempt in range(50):
                    if server.poll() is not None:
                        log.seek(0)
                        raise RuntimeError(log.read())
                    try:
                        status_code, status = helpers.http(base, '/api/status')
                        break
                    except urllib.error.URLError:
                        if attempt == 49:
                            raise
                        time.sleep(0.1)
                assert status_code == 200 and status['stats']['total_segments'] == inventory_count
                assert status['task_scope']['total_segments'] == len(expected)
                actual = set()
                for page in range(1, (len(expected) + 199) // 200 + 1):
                    code, data = helpers.http(base, f'/api/segments?selection=task&page={page}&page_size=200')
                    assert code == 200 and data['total'] == len(expected)
                    ids = {row['id'] for row in data['items']}
                    assert len(ids) == len(data['items']) and not actual.intersection(ids)
                    actual.update(ids)
                assert actual == expected
                code, data = helpers.http(base, '/api/segments?selection=inventory&page_size=1')
                assert code == 200 and data['total'] == inventory_count
                code, data = helpers.http(base, f'/api/segments?selection=task&file_id={excluded_file}')
                assert code == 200 and data['total'] == 0
                code, data = helpers.http(base, '/api/segments?selection=task&stage=0&search=S1017B&page_size=200')
                assert code == 200 and data['total'] == 95 and all(row['id'] in expected for row in data['items'])
                try:
                    urllib.request.urlopen(base + '/api/segments?selection=invalid', timeout=10)
                except urllib.error.HTTPError as error:
                    assert error.code == 400 and 'invalid' in error.read().decode()
                else:
                    raise AssertionError('Invalid selection was accepted')
                code, data = helpers.http(base, '/api/review-queue?selection=task')
                assert code == 200 and data['total'] == status['task_scope']['pending_review_segments']
                mcp = subprocess.Popen(command + ['mcp', '--project-id', args.project], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
                try:
                    helpers.rpc(mcp, 1, 'initialize', {'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'scope-integration','version':'1'}})
                    assert mcp.stdin
                    mcp.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
                    mcp.stdin.flush()
                    def call(request_id: int, name: str, arguments: dict[str, str | int]) -> dict[str, object]:
                        response = helpers.rpc(mcp, request_id, 'tools/call', {'name':name,'arguments':arguments})
                        return response.get('structuredContent') or json.loads(response['content'][0]['text'])
                    current = call(2, 'project_status', {'project_id':args.project})
                    assert current['total'] == inventory_count and current['task_scope']['total_segments'] == len(expected)
                    page = call(3, 'list_segments', {'project_id':args.project,'offset':0,'limit':50})
                    assert page['total'] == len(expected) and all(row['id'] in expected for row in page['items'])
                    page = call(4, 'list_segments', {'project_id':args.project,'offset':0,'limit':1,'selection':'inventory'})
                    assert page['total'] == inventory_count
                    denial = call(5, 'list_segments', {'project_id':'wrong-project','offset':0,'limit':1,'selection':'task'})
                    assert denial['code'] == 'project_mismatch'
                    assert fingerprint(root / 'project.db') == baseline
                    zh = root / 'empty-zh'
                    zh.mkdir()
                    subprocess.run(command + ['extract', '--project-id',args.project,'--kr',source_dir,'--zh',str(zh)], capture_output=True, text=True, check=True, timeout=180)
                    code, stale = helpers.http(base, '/api/segments?selection=task&page_size=1')
                    assert code == 409 and 'stale' in stale['message']
                    denial = call(6, 'list_segments', {'project_id':args.project,'offset':0,'limit':1,'selection':'task'})
                    assert denial['code'] == 'conflict' and 'stale' in denial['message']
                    code, inventory = helpers.http(base, '/api/segments?selection=inventory&page_size=1')
                    assert code == 200 and inventory['total'] == inventory_count
                finally:
                    helpers.stop(mcp)
            finally:
                helpers.stop(server)
    assert fingerprint(args.database) == before
    report = {'passed':True,'scope_segments':len(expected),'scope_files':len(manifest['files']),'inventory_segments':inventory_count,
              'all_selected_ids_paged_and_matched':True,'source_reimport_rejected_stale_scope':True,'fresh_stdio_mcp_verified':True,
              'production_rows_unchanged':True,'production_fingerprint':before,'rejected_imports':rejected}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps(report,ensure_ascii=False,indent=2))


if __name__ == '__main__':
    main()
