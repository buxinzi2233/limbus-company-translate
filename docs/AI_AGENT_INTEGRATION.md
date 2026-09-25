# LCT (Limbus Company Translate) AI 原生接入与自动化调用规范

> **版本**：v0.2.0-LCT  
> **适用代理**：Google Antigravity, Claude Code, Cursor, Cline 及各类具备 Bash / Tool Calling 能力的自主 AI Agent。  
> **核心原则**：纯净结构化流交互（全命令原生支持 `--json`）、RFC-6901 指针 100% 格式无损防污染、L01~L08 门禁闭环自愈、人机权责分立（AI 高速作业，人类大盘监督）。

---

## 一、 顶层架构与双模协同

LCT 为 AI Agent 提供了两种标准的接入模式：

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                          LCT AI INTEGRATION MODES                           │
├─────────────────────────────────────────────────────────────────────────────┤
│  [MODE 1: CLI-First (流水线批处理模式)]                                     │
│  AI Agent ──(CLI --json)──► lct extract ──► tickets/pending.jsonl           │
│                                                       │ (AI 批量翻译)       │
│                                                       ▼                     │
│  AI Agent ◄──(JSON AST)─── lct qa ◄─────── tickets/translated.jsonl        │
│      │ (自检自愈)                                                           │
│      └────────────────────► lct inject ──► 游戏 JSON 工作区                 │
│                                                                             │
│  [MODE 2: Daemon + Live Browser (长程人机协同模式)]                         │
│  AI Agent ──► 执行 `lct ui --port 3000` (瞬间自动在人类浏览器弹出终末地大盘)│
│  AI Agent ──► 后台通过 HTTP REST API 进行词条存取与反向查词                 │
│  Human    ──► 浏览器前台实时监控大盘跳动，并聚焦 1% 疑难词条裁决            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 二、 模式一：CLI 批处理全流程调用协议

所有命令均支持 `--json` 参数，返回纯净的机器可读 JSON，标准输出不掺杂任何装饰性文字。

### 1. 扫描增量差分并提取工单 (`lct extract`)
当游戏发布新版本或新赛季时，AI 执行增量提取：
```bash
./target/release/lct --json extract \
  --kr "/path/to/baseline-KR" \
  --zh "/path/to/workspace/LLC_zh-CN" \
  --ticket "tickets/batch_01.jsonl" \
  --auto-fill-tm true
```
**JSON 返回响应**：
```json
{
  "scanned_files": 2022,
  "total_segments": 144181,
  "already_translated": 143843,
  "tm_exact_filled": 0,
  "pending_untranslated": 338,
  "elapsed_ms": 8651
}
```
**工单 `tickets/batch_01.jsonl` 单行示例**：
```json
{
  "file": "AbEvents-a1c8p2.json",
  "pointer": "/dataList/0/title",
  "speaker": null,
  "source_kr": "막힌_길_장치_조작",
  "target_zh": null,
  "tm_match": null,
  "tm_tier": null,
  "stage": "Untranslated"
}
```

---

### 2. 遇到专有名词反向检索 TM 记忆库 (`lct search`)
当 AI 遇到世界观术语、代号或不确定的语法时，毫秒级反向检索上游 14.3 万条权威记忆库：
```bash
./target/release/lct --json search "균열" --limit 5
```
**JSON 返回响应**：
```json
{
  "query": "균열",
  "total_found": 5,
  "tm_entries": [
    {
      "id": 1024,
      "source_kr": "거울 조각은 사람의 마음에게서 무수한 균열을 이끌어내죠.",
      "target_zh": "镜子的碎片会使人类的心灵产生无数的裂痕。",
      "file_scope": "StoryData/S645B.json",
      "speaker": "파우스트",
      "source_origin": "zero-assoc-20260921",
      "priority": 100
    }
  ]
}
```

---

### 3. 翻译质量门禁检查与自愈 (`lct qa` & `lct fix`)
AI 在草拟出中文后，必须先通过质量门禁：
```bash
./target/release/lct --json qa "使目标陷入[沉沦]状态，层数维持1～3层" --kr "..."
```
**违规拦截响应 (Exit Code 0, passed = false)**：
```json
{
  "passed": false,
  "issue_count": 2,
  "issues": [
    {
      "rule_id": "L02",
      "severity": "Fatal",
      "message": "严禁使用全角波浪号 '～'，会导致 Unity TextMeshPro 方框乱码！",
      "suggested_fix": "将 '～' 替换为半角 '~'"
    },
    {
      "rule_id": "L04",
      "severity": "Warning",
      "message": "战斗关键词 [沉沦] 后缺少半角空格，会导致词条悬浮窗无法触发！",
      "suggested_fix": "[沉沦] "
    }
  ]
}
```
**AI 自动纠偏执行**：
```bash
./target/release/lct --json fix "使目标陷入[沉沦]状态，层数维持1～3层"
```
**返回**：
```json
{
  "original": "使目标陷入[沉沦]状态，层数维持1～3层",
  "fixed": "使目标陷入[沉沦] 状态，层数维持1~3层"
}
```

---

### 4. 无损安全原地注入回填 (`lct inject`)
工单校验完毕后，AI 执行原子回填：
```bash
./target/release/lct --json inject \
  --workspace "/path/to/workspace/LLC_zh-CN" \
  --kr "/path/to/baseline-KR" \
  --ticket "tickets/batch_01_translated.jsonl"
```
**JSON 返回响应**：
```json
{
  "modified_files": 1,
  "injected_segments": 338,
  "skipped_untranslated": 0,
  "qa_violations": 0,
  "elapsed_ms": 42
}
```

---

## 三、 模式二：Daemon + 自动拉起浏览器协同

AI 可以启动 LCT 守护进程。**工具启动瞬间会自动调用宿主系统默认浏览器弹出 UI 态势大盘**，同时后台提供极速 REST 服务：

```bash
# AI 启动后台服务 (自动打开浏览器)
./target/release/lct ui --port 3000
```

### 核心 REST API 清单 (供 AI 在后台发起 HTTP 请求)

| 端点 | 方法 | 作用 | 请求参数 / Body |
| :--- | :---: | :--- | :--- |
| `/api/status` | `GET` | 获取大盘宏观态势与分模块进度 | 无 |
| `/api/files` | `GET` | 列出全量文件及各自翻译进度 | 无 |
| `/api/segments` | `GET` | 分页拉取真实词条列表 | `?page=1&page_size=50&stage=0` (0:生肉, 1:TM, 4:已审) |
| `/api/segments/update`| `POST`| 远程写入并锁定单条翻译成果 | `{"id": 101, "target_zh": "...", "stage": 4}` |
| `/api/tm/search` | `GET` | 跨 14.3 万条 TM 数据库全文搜索 | `?q=契约&limit=20` |
| `/api/qa/check` | `POST`| 门禁与数值/极性语义检验 | `{"text": "...", "kr": "..."}` |
| `/api/qa/fix` | `POST`| 确定性安全自愈修复 | `{"text": "..."}` |

---

## 四、 AI Agent Tool Definition (标准工具契约)

若在支持 MCP 或原生 Function Calling 的 Agent 环境中，可直接将以下 JSON 注册为 Agent 工具：

```json
{
  "name": "lct_tool",
  "description": "边狱公司工业级本地化工具 (LCT)。支持增量生肉提取、TM记忆库毫秒级检索、质量门禁质检自愈、安全无损原地注入以及态势大盘拉起。",
  "parameters": {
    "type": "object",
    "properties": {
      "action": {
        "type": "string",
        "enum": ["status", "extract", "search", "qa_check", "qa_fix", "inject", "start_ui"],
        "description": "执行的本地化动作"
      },
      "query": { "type": "string", "description": "搜索关键词 (用于 search 动作)" },
      "text": { "type": "string", "description": "待检查或修复的中文文本 (用于 qa_check / qa_fix)" },
      "kr_text": { "type": "string", "description": "对应的韩文原文 (用于 qa_check 跨语言数值与极性对比)" },
      "ticket_path": { "type": "string", "description": "工单 JSONL 路径" },
      "port": { "type": "integer", "description": "WebUI 端口 (默认 3000)" }
    },
    "required": ["action"]
  }
}
```
