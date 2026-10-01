"""Compare LCT-extracted source identities without assigning translations by array position."""
import argparse
from collections import Counter
from dataclasses import dataclass, asdict
import json
from pathlib import Path
import sqlite3


@dataclass(frozen=True)
class Field:
    path: str
    identity: str
    pointer: str
    text: str


def fields(database: Path, project: str) -> dict[tuple[str, str], Field]:
    with sqlite3.connect(database.resolve().as_uri()+"?mode=ro", uri=True) as conn:
        rows = conn.execute('''SELECT f.relative_path,s.source_identity,s.key_path,s.source_kr FROM segments s
            JOIN files f ON f.id=s.file_id WHERE s.project_id=? AND s.active=1 AND f.active=1''',(project,))
        result: dict[tuple[str,str],Field] = {}
        for path,identity,pointer,text in rows:
            if not identity:
                raise ValueError(f"Missing persisted source identity: {path}:{pointer}")
            key = (path,identity)
            if key in result:
                raise ValueError(f"Ambiguous persisted source identity: {key}")
            result[key] = Field(path,identity,pointer,text)
        return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--before",type=Path,required=True)
    parser.add_argument("--before-project",required=True)
    parser.add_argument("--after",type=Path,required=True)
    parser.add_argument("--after-project",required=True)
    parser.add_argument("--file-audit",type=Path,required=True)
    parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args()
    before,after=fields(args.before,args.before_project),fields(args.after,args.after_project)
    audit=json.loads(args.file_audit.read_text())['historical_delta']
    shared=set(audit['modified'])
    changes=[]
    for key in sorted(before.keys() | after.keys()):
        old,new=before.get(key),after.get(key)
        if old is not None and new is not None and old.text==new.text:
            continue
        field=new if new is not None else old
        kind='added' if old is None else 'removed' if new is None else 'changed'
        changes.append({'kind':kind,'path':field.path,'source_identity':field.identity,
            'old_pointer':old.pointer if old else None,'new_pointer':new.pointer if new else None,
            'old_kr':old.text if old else None,'new_kr':new.text if new else None,
            'identity_scope':'snapshot_bound' if 'SnapshotEntry' in field.identity else 'persisted_identity',
            'source_file_group':'modified_shared' if field.path in shared else 'new_file' if field.path in audit['added'] else 'other'})
    counts=Counter(row['kind'] for row in changes)
    assert len(after)-len(before)==counts['added']-counts['removed']
    result={'before_segments':len(before),'after_segments':len(after),'counts':dict(counts),
        'shared_counts':dict(Counter(row['kind'] for row in changes if row['source_file_group']=='modified_shared')),
        'shared_files':[{'path':path,'counts':dict(Counter(row['kind'] for row in changes if row['path']==path))} for path in sorted(shared)],
        'identity_policy':'Uses LCT source_identity including typed record IDs and snapshot-bound ambiguous arrays. Positional identities are structural comparisons only, not semantic pairing or TM approval.',
        'changes':changes}
    args.output.write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
    print(json.dumps({key:value for key,value in result.items() if key not in ('changes','shared_files')},ensure_ascii=False,indent=2))


if __name__=='__main__':
    main()
