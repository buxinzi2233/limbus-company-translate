# LCT · Limbus Company Translate

**[English](README_EN.md)** | 简体中文

边狱公司（Limbus Company）KR → ZH-CN 工业级本地化工程平台：以翻译记忆库为核心的 Rust 工作区，提供命令行管线、本地 WebUI 态势大盘与 MCP 智能体接口。

> 语言的边界，不是理解的边界。

## 这是什么

LCT 把"游戏文本汉化"当作一条有门禁的工程流水线，而不是一次性脚本：

- **索引**双语基准语料，反哺 SQLite FTS5 全文翻译记忆库（TM）
- **提取**生肉与现有汉化的增量差异，产出工单与待译条目
- **注入**以 RFC 6901 JSON Pointer 原地无损回填，不动无关字段
- **门禁**对每个翻译单元执行 L 级规则稽核（波浪号、占位符、间距、术语一致性……），AI 草稿必须过门禁才能入库，且永远不会被标记为"人工已审校"
- **审校**在本地 WebUI 中由人完成：双语工作台、语料检索、来源审计、待审队列

工具链以 AI 调用为主（CLI 执行经授权的索引/提取/文件操作，MCP 提供项目读取、QA 与草稿提交），WebUI 用于人工观察、核对和审校。

## 三个入口

### 1. CLI

```bash
lct init                       # 初始化工作区
lct index                      # 索引双语基准语料，反哺 TM
lct extract                    # 提取增量待译条目（可导出工单 JSONL）
lct inject                     # 安全注入回填至游戏 JSON
lct qa                         # 本地化门禁与语义一致性稽核
lct fix                        # 确定性安全自动修复
lct search <词>                # 毫秒级反向实机台词全文搜索
lct scope-import / scope-status   # 任务范围校验与读取
lct draft-grant / draft-revoke    # AI 草稿授权签发 / 即时吊销
lct restore                    # 显式备份恢复，不覆盖无关改动
lct ui                         # 启动本地 WebUI 态势大盘
lct mcp                        # 以 MCP stdio 提供受限项目工具
```

### 2. WebUI（本地态势大盘）

```bash
lct ui --project-id review --port 3000
```

仅监听本机回环地址。五个工作面：项目总览（观测仪器）、双语工作台、语料检索、来源与审计、待审队列。UI 经 rust-embed 压入单个可执行文件，分发无需携带静态资源。

### 3. MCP（AI 智能体）

```bash
lct mcp --project-id review
```

面向本机单用户的显式项目：AI 可读取项目、运行 QA、提交草稿——草稿必须事先获得人工签发的 `draft-grant` 授权，且提交后仍进入待审队列，由人最终决定。

## 构建

前端先行，发布版将 `ui/dist` 嵌入二进制：

```bash
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
cargo build --release --locked -p loc-cli
./target/release/lct ui --no-open
```

要求：Rust 1.89+，Node.js + pnpm。目标平台为 Linux x86_64 本机单用户工作区。

## 仓库结构

```
crates/
  loc-core     核心模型、SQLite 存储、审计、术语
  loc-parser   AST 提取器 / RFC 6901 注入器 / 资源分类
  loc-tm       翻译记忆库引擎
  loc-qa       门禁规则与自动修复
  loc-server   本地 HTTP API 与嵌入式资产
  loc-cli      lct 命令行
  loc-mcp      MCP stdio 服务器
ui/            React + Vite 态势大盘（莱茵生命美术方向）
docs/          接口说明、生产就绪清单、UI 美术方向
scripts/       发布与验证脚本
```

## 设计红线

- **本机单用户**：无远程身份认证、无多用户项目选择、无远程 MCP；不要用网络代理暴露服务
- **人工终审**：AI 草稿永远进入待审队列；门禁通过不等于人工签收
- **无损写回**：注入不触碰无关字段；恢复只还原备份范围内的内容
- **减少动态效果**：所有 UI 动效遵守系统 `prefers-reduced-motion` 与界面开关双控

更多接口与安全边界见 [docs/AI_AGENT_INTEGRATION.md](docs/AI_AGENT_INTEGRATION.md)；美术方向见 [docs/UI_ART_DIRECTION.md](docs/UI_ART_DIRECTION.md)。

## 许可

[MIT](LICENSE) · Antigravity & Hackimi Localization Team
