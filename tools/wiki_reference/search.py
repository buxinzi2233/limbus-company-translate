"""Search the standalone reference database with source attribution in every result."""

import argparse
import json
from pathlib import Path

from store import search


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db",required=True,type=Path)
    parser.add_argument("--query",required=True)
    parser.add_argument("--limit",required=True,type=int)
    args = parser.parse_args()
    print(json.dumps(search(args.db,args.query,args.limit),ensure_ascii=False,indent=2))


if __name__ == "__main__":
    main()
