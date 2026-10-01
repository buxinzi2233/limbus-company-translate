#!/usr/bin/env python3
"""Exercise advisory QA through real HTTP and MCP without persisting translations."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import urllib.request
import urllib.parse


def fingerprint(database: Path) -> str:
    digest = hashlib.sha256()
    with sqlite3.connect(database.resolve().as_uri() + '?mode=ro', uri=True) as conn:
        for row in conn.execute('SELECT id,source_kr,target_zh,stage,review_confirmed,updated_at FROM segments ORDER BY id'):
            digest.update(json.dumps(row, ensure_ascii=False).encode())
    return digest.hexdigest()


def verify(binary: Path, root: Path, reference: Path, project: str, base: str) -> dict[str, object]:
    spec = importlib.util.spec_from_file_location('reference_checks', Path(__file__).with_name('verify-references.py'))
    assert spec and spec.loader
    helpers = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(helpers)
    before = fingerprint(root / 'project.db')
    reference_before = hashlib.sha256(reference.read_bytes()).hexdigest()
    with sqlite3.connect((root / 'project.db').resolve().as_uri() + '?mode=ro', uri=True) as conn:
        combat = conn.execute("SELECT s.id,s.source_kr FROM segments s JOIN files f ON s.file_id=f.id WHERE s.project_id=? AND s.active=1 AND f.relative_path='Skills.json' AND s.source_kr='자신의 [Breath]이 4 이상이면, 코인 위력 +2'", (project,)).fetchone()
        story = conn.execute("SELECT s.id FROM segments s JOIN files f ON s.file_id=f.id WHERE s.project_id=? AND s.active=1 AND f.relative_path LIKE 'StoryData/%' LIMIT 1", (project,)).fetchone()
        summary = conn.execute("SELECT s.id FROM segments s JOIN files f ON s.file_id=f.id WHERE s.project_id=? AND s.active=1 AND f.relative_path='BattleKeywords.json' AND s.key_path LIKE '%/summary' AND s.source_kr='호흡 위력 얻음, 호흡 횟수 증가' LIMIT 1", (project,)).fetchone()
        stats = conn.execute("SELECT s.id FROM segments s JOIN files f ON s.file_id=f.id WHERE s.project_id=? AND s.active=1 AND f.relative_path='RPGSystem/rpg-loc-item-common-a1c10p1.json' AND s.key_path='/dataList/14/statText' AND s.source_kr LIKE '%출혈%'", (project,)).fetchone()
        name = conn.execute("SELECT s.id FROM segments s JOIN files f ON s.file_id=f.id WHERE s.project_id=? AND s.active=1 AND f.relative_path='BattleKeywords.json' AND s.key_path LIKE '%/name' AND s.source_kr='호흡' LIMIT 1", (project,)).fetchone()
    assert combat and story and summary and stats and name, 'Required real source contexts are absent' 
    cases = [
        (combat[0], '自身的呼吸法不少于4时，硬币威力+2', ['T02'], True),
        (combat[0], '自身的呼吸法 不少于4时，硬币威力+2', [], True),
        (combat[0], '自身的[Breath]不少于4时，硬币威力+2', [], True),
        (combat[0], '自身的集中不少于4时，硬币威力+2', ['T01'], True),
        (story[0], '呼吸法', [], False),
        (summary[0], '获得呼吸法威力，增加呼吸法次数', ['T02'], True),
        (stats[0], '流血威力2，流血次数2，体力+1', ['T02'], True),
        (stats[0], '流血 威力2，流血 次数2，体力+1', [], True),
        (name[0], '呼吸法', [], False),
    ]
    results = []
    for segment, target, expected, applicable in cases:
        req = urllib.request.Request(base + '/api/qa/terminology', data=json.dumps({'id':segment,'target_zh':target}).encode(), headers={'Content-Type':'application/json','Origin':base})
        with urllib.request.urlopen(req, timeout=15) as response:
            result = json.load(response)
        assert [x['rule_id'] for x in result['advice']] == expected, result
        assert result['applicable'] == applicable and result['blocking'] is False
        assert result['available_links'] == 7
        results.append({'id':segment,'target':target,'report':result})
    with urllib.request.urlopen(base + '/api/references/search?' + urllib.parse.urlencode({'q':'호흡','limit':8}), timeout=15) as response:
        found = json.load(response)
    assert [link['keyword_id'] for link in found['name_links']] == ['Breath']
    with tempfile.TemporaryFile(mode='w+') as log:
        process = subprocess.Popen([str(binary), '--data-dir',str(root),'--reference-db',str(reference),'mcp','--project-id',project], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True, bufsize=1)
        try:
            helpers.rpc(process,1,'initialize',{'protocolVersion':'2025-11-25','capabilities':{},'clientInfo':{'name':'terminology-integration','version':'1'}})
            assert process.stdin
            process.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
            process.stdin.flush()
            raw = helpers.rpc(process,2,'tools/call',{'name':'check_translation','arguments':{'project_id':project,'id':combat[0],'target_zh':cases[0][1]}})
            assert not raw.get('isError'), raw
            payload = json.loads(raw['content'][0]['text'])
            assert payload['terminology']['advice'][0]['rule_id'] == 'T02', payload
            assert payload['passed'] == (len(payload['issues']) == 0), payload
        finally:
            helpers.stop(process)
    assert fingerprint(root / 'project.db') == before
    assert hashlib.sha256(reference.read_bytes()).hexdigest() == reference_before
    return {'status':'passed','http_cases':results,'mcp_check':payload,'korean_name_search':found['name_links'], 'project_segments_unchanged':True,'reference_unchanged':True}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--data-dir',type=Path,required=True)
    parser.add_argument('--reference-db',type=Path,required=True)
    parser.add_argument('--project-id',required=True)
    parser.add_argument('--base-url',required=True)
    parser.add_argument('--output',type=Path,required=True)
    args = parser.parse_args()
    result = verify(args.binary.resolve(),args.data_dir.resolve(),args.reference_db.resolve(),args.project_id,args.base_url)
    args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print('Real HTTP/MCP terminology checks passed; no translations persisted')


if __name__ == '__main__':
    main()
