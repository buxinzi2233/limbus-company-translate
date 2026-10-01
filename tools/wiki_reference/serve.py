"""Local-only capture import and reference search; never writes an LCT work database."""

import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
from pathlib import Path
from urllib.parse import parse_qs, urlsplit

from model import CaptureError
from store import import_capture, initialize, search, status

HTML = """<!doctype html><html lang="zh-CN"><meta charset="utf-8"><title>LCT · 边狱巴士 Wiki 参考库</title>
<style>body{background:#151a1c;color:#e7ece8;font:16px system-ui;max-width:1100px;margin:40px auto;padding:0 24px}h1{font-size:26px}p{color:#b9c8c0}input,textarea,button{background:#222b2b;color:inherit;border:1px solid #657c71;padding:10px;font:inherit}input{width:60%}textarea{width:96%;height:100px}button{cursor:pointer}pre{white-space:pre-wrap;overflow-wrap:anywhere;background:#202627;padding:20px;max-height:650px;overflow:auto}details{margin-top:30px}a{color:#a7d4ba}</style>
<h1>边狱巴士 Wiki · 翻译参考库</h1><p>只收录 limbuscompany.huijiwiki.com · 社区参考不是已审校译文或双语 TM</p>
<pre id="status" data-testid="wiki-status">正在读取</pre>
<form id="search-form"><input id="query" data-testid="wiki-query" aria-label="查询术语或正文" placeholder="输入：呼吸法、震颤、李箱…" required><button data-testid="wiki-search">检索</button></form>
<pre id="results" data-testid="wiki-results">结果附页面来源、版本及许可；词条用语须结合当前原文核验。</pre>
<details><summary>导入已读取的公开 API 响应</summary><p>仅接受本站 main namespace 页面及匹配修订号的原文/渲染响应；不会连接游戏或翻译库。</p>
<textarea id="capture" data-testid="wiki-capture" aria-label="版本化页面采集 JSON"></textarea><button id="import" data-testid="wiki-import">验证并导入</button><pre id="receipt" data-testid="wiki-receipt">尚未导入</pre></details>
<script>
const show=(id,value)=>document.getElementById(id).textContent=JSON.stringify(value,null,2);
const refresh=async()=>{const r=await fetch('/api/status');show('status',await r.json())};
document.getElementById('search-form').addEventListener('submit',async e=>{e.preventDefault();const r=await fetch('/api/search?q='+encodeURIComponent(document.getElementById('query').value));show('results',await r.json())});
document.getElementById('import').addEventListener('click',async()=>{const b=document.getElementById('import');b.disabled=true;try{const r=await fetch('/api/import',{method:'POST',headers:{'Content-Type':'application/json'},body:document.getElementById('capture').value});show('receipt',await r.json());await refresh()}catch(e){show('receipt',{error:String(e)})}finally{b.disabled=false}});
refresh().catch(e=>show('status',{error:String(e)}));
</script></html>"""


def handler_for(db: Path, captures: Path, port: int) -> type[BaseHTTPRequestHandler]:
    class Handler(BaseHTTPRequestHandler):
        """HTTP connector for explicitly selected local reference storage."""

        def reply(self, status_code: int, data: bytes, content_type: str) -> None:
            self.send_response(status_code)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.end_headers()
            self.wfile.write(data)

        def json_reply(self, status_code: int, value: object) -> None:
            self.reply(status_code, json.dumps(value, ensure_ascii=False).encode(), "application/json; charset=utf-8")

        def local_request(self) -> bool:
            return self.headers.get("Host") == f"127.0.0.1:{port}"

        def do_GET(self) -> None:
            if not self.local_request():
                self.json_reply(403,{"error":"Only the configured loopback host is accepted"})
                return
            target = urlsplit(self.path)
            if target.path == "/":
                self.reply(200,HTML.encode(),"text/html; charset=utf-8")
            elif target.path == "/api/status":
                self.json_reply(200,status(db))
            elif target.path == "/api/search":
                try:
                    query = parse_qs(target.query).get("q", [""])[0]
                    self.json_reply(200,search(db,query,20))
                except ValueError as error:
                    self.json_reply(400,{"error":str(error)})
            else:
                self.json_reply(404,{"error":f"Unknown path {target.path}"})

        def do_POST(self) -> None:
            if not self.local_request() or self.headers.get("Origin") != f"http://127.0.0.1:{port}":
                self.json_reply(403,{"error":"Import requires same-origin loopback access"})
                return
            if self.path != "/api/import":
                self.json_reply(404,{"error":f"Unknown path {self.path}"})
                return
            try:
                length = int(self.headers.get("Content-Length", "0"))
                if not 0 < length <= 8_000_000:
                    raise CaptureError("Import body must contain 1-8000000 bytes")
                payload = self.rfile.read(length).decode("utf-8")
                self.json_reply(200,import_capture(db,captures,payload))
            except (ValueError, UnicodeError) as error:
                self.json_reply(422,{"error":str(error)})

    return Handler


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--db",required=True,type=Path)
    parser.add_argument("--captures",required=True,type=Path)
    parser.add_argument("--port",required=True,type=int)
    args = parser.parse_args()
    if args.db.name == "project.db":
        parser.error("Refusing to use an LCT work database as a reference database")
    initialize(args.db)
    server = HTTPServer(("127.0.0.1",args.port),handler_for(args.db,args.captures,args.port))
    print(f"Wiki reference library: http://127.0.0.1:{args.port}",flush=True)
    server.serve_forever()


if __name__ == "__main__":
    main()
