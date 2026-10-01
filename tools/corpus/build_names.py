"""Prepare a narrowly verified name-only corpus without changing original source files."""
import argparse
from dataclasses import dataclass, asdict
import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'wiki_reference'))
from terms import LINKS, entries


@dataclass(frozen=True)
class PairEvidence:
    keyword_id: str
    source_kr: str
    target_zh: str
    old_kr_pointer: str
    current_kr_pointer: str
    zh_pointer: str
    verification: str
    human_approved: bool


def build(old_kr: Path, current_kr: Path, zh: Path, output: Path) -> None:
    older, current, translated = entries(old_kr), entries(current_kr), entries(zh)
    pairs: list[PairEvidence] = []
    for link in LINKS:
        old_index, old_name = older[link.keyword_id]
        current_index, current_name = current[link.keyword_id]
        zh_index, zh_name = translated[link.keyword_id]
        if old_name != current_name or current_name != link.source_kr or zh_name != link.target_zh:
            raise ValueError(f'Name identity/version mismatch for {link.keyword_id}: before={old_name!r}, current={current_name!r}, Chinese={zh_name!r}')
        pairs.append(PairEvidence(link.keyword_id,current_name,zh_name,f'/dataList/{old_index}/name',f'/dataList/{current_index}/name',f'/dataList/{zh_index}/name','unique_typed_id_field_and_name_semantics_checked',False))
    output.mkdir(parents=True, exist_ok=True)
    for language in ('kr','zh'):
        directory = output / language
        directory.mkdir(exist_ok=True)
        document = {'dataList':[{'id':p.keyword_id,'name':p.source_kr if language=='kr' else p.target_zh} for p in pairs]}
        (directory/'BattleKeywords.json').write_text(json.dumps(document,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    evidence = {'purpose':'Name-only source-attributed reference TM; not descriptions, project translations or human approval','source_origin':'zeroasso-2026092102-names-ai-verified-unreviewed','priority':10,'modification':'Selected only seven name fields by unique ID; kept spelling unchanged; all other fields omitted','sources':[{'path':str(p.resolve()),'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in (old_kr,current_kr,zh)],'zero_license':'CC BY-NC-SA 4.0; see references/zeroasso/zh-CN/Info/LICENSE','source_rights':'Project Moon Korean source; no relicensing','evidence_links':'data/todo-completion/term-links.json','pairs':[asdict(p) for p in pairs]}
    (output/'evidence.json').write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'verified_names':len(pairs),'output':str(output)},ensure_ascii=False))


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--before-kr',type=Path,required=True)
    parser.add_argument('--current-kr',type=Path,required=True)
    parser.add_argument('--zh',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    build(args.before_kr,args.current_kr,args.zh,args.output)


if __name__=='__main__':
    main()
