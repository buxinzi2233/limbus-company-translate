<div align="center">

# LCT · Limbus Company Translate

**让原文与译文之间的每一步，都有据可循。**

面向《边狱公司》的本地 KR → ZH-CN 翻译工作站<br>
Rust CLI · 双语审校 WebUI · MCP 智能体接口 · 离线参考语料

**简体中文** · [English](README_EN.md) · [下载发布包](https://github.com/buxinzi2233/limbus-company-translate/releases) · [接口文档](docs/AI_AGENT_INTEGRATION.md)

</div>

![LCT 项目总览：任务范围、翻译覆盖率、分域进度和待审数量](docs/images/overview.jpg)

> 上图及下方工作台截图来自隔离的 `readme-demo` 示例项目，不含生产翻译数据，也不代表真实汉化进度；检索截图使用随包分发的公开 Wiki 参考快照。

## 不只是填写译文

LCT 将原文导入、参考检索、翻译草稿、人工审校和 JSON 写回串成可追踪的本地工作流程。CLI 负责工程操作，WebUI 负责人工核对，MCP 为智能体提供受限的项目接口。

| 能力 | 具体用途 |
| --- | --- |
| **双语工作台** | 并排查看韩文与中文，按任务、资源分区和阶段筛选，保留上下文与字段位置 |
| **参考检索** | 检索项目词条、翻译记忆和 Wiki；展示来源、修订号、署名与许可，不混淆参考与已审核译文 |
| **增量导入** | 识别新增、变更和删除的原文；保护已有翻译并标明需要复核的内容 |
| **QA 与人工审校** | 检查占位符、数字、富文本及格式规则；AI 草稿通过 QA 也不会自动变成人工签收 |
| **写回与恢复** | 按 JSON Pointer 回填，保留无关字段，提供备份、恢复和操作审计 |

### 界面预览

<details>
<summary><strong>双语工作台 · 原文、上下文和译文在同一处审校</strong></summary>

![双语工作台：筛选后的单条示例、韩中对照及保存并审校操作](docs/images/workbench.jpg)

示例译文仍标为“导入译文待审”。已导入、有译文、QA 通过和人工确认是不同状态。

</details>

<details>
<summary><strong>语料检索 · 将社区参考与翻译记忆分开呈现</strong></summary>

![语料检索：呼吸法的 Wiki 参考、页面修订号、署名和许可](docs/images/references.jpg)

截图引用边狱公司中文维基“呼吸法”页面（修订 104445）；原作者见页面历史，引用内容沿用 CC BY-SA 4.0 及页面单独声明。完整来源和修改说明保存在参考库及 [Wiki 许可记录](corpus/licenses/wiki-source-license.json) 中。

</details>

## 快速开始

### 下载即用：程序 + 前端 + 语料

从 [Releases](https://github.com/buxinzi2233/limbus-company-translate/releases) 下载 `lct-linux-x86_64.tar.gz` 和同名 `.sha256` 文件，放在同一目录。

**预编译包要求 Linux x86_64、glibc 2.39+ 及 `libgcc_s.so.1`。** 恢复语料还需要 Python 3.11+，其 SQLite 模块须支持 FTS5。无需安装 Rust、Node.js 或 pnpm；其他系统尚未提供预编译包，较旧 Linux 运行库可尝试在本机从源码构建，但未经跨发行版兼容性认证。

```bash
sha256sum --check lct-linux-x86_64.tar.gz.sha256
tar -xzf lct-linux-x86_64.tar.gz
cd lct-linux-x86_64
sha256sum --check SHA256SUMS

python3 scripts/setup-workspace.py \
  --binary "$PWD/lct" --workspace "$HOME/lct-review" \
  --project-id review --name "我的本地化项目" --port 3000
"$HOME/lct-review/start-ui.sh"
```

在浏览器打开 **http://127.0.0.1:3000**；终端按 `Ctrl+C` 停止界面服务。初始化全程离线、逐文件校验，拒绝覆盖已有工作区；不修改任何全局 AI 客户端设置，也不会自动签发写入许可。

### 把 AI 接进来

1. 将新工作区 `mcp.json` 中的 **command 与 args** 配到支持 stdio MCP 的 AI 客户端；文件使用常见 `mcpServers` JSON 外壳，客户端若使用其他格式，应保留这些参数并按其格式填写。
2. 重连客户端，将工作区的 `AGENTS.md` 交给 AI；项目 Skill 与接口文档已在发布包中。仅接入 MCP 的客户端也能通过工具读取研究全文，不要求模型访问本机文件。
3. 验证发现 **13 个工具**，先调用 `project_status`（`project_id: "review"`），再调用 `reference_status`、`search_references`（如“呼吸法”）、`search_corpus`（如“经理”）、`list_research`（总数 65）、`search_research`、`read_research`。
4. 现在即可只读查阅公开中文语料、Wiki 和角色/文风研究。开始自己的翻译时，仍须自行提供合法取得的完整韩文源快照，由人确认导入、任务范围、执行器隔离与草稿许可；AI 不能替人审批或写入游戏文件，具体步骤见[接口说明](docs/AI_AGENT_INTEGRATION.md)。

生成文件：`data/`（新工作库）、`references/`（只读语料与中文 JSON）、`mcp.json`（接入配置）、`start-ui.sh`（界面启动器）、`workspace.json`（明确的项目与路径）、`AGENTS.md`（AI 交接说明）。配置使用绝对路径，**不要移动或删除解压后的工具目录**；不包含模型服务账号或 API 密钥，需要用户在自己的 AI 客户端配置。

首次库存为零是正常状态，参考资料不等于待译任务或完成量。只想恢复语料、不创建项目时，仍可单独运行 `python3 scripts/prepare-references.py /absolute/new-reference-directory`。

### 从源码构建

准备 Rust/Cargo、Node.js 和 pnpm。工作区声明 Rust 1.89；本次发布实际使用 Rust 1.98.1、Node.js 26.10.0、pnpm 11.26.0 构建，未验证最低工具链组合。

```bash
git clone https://github.com/buxinzi2233/limbus-company-translate.git
cd limbus-company-translate
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
cargo build --release --locked -p loc-cli
```

前端必须先构建：发布程序将 `ui/dist` 嵌入可执行文件。随后按上方初始化步骤操作，将 `--binary "$PWD/lct"` 替换为 `--binary "$PWD/target/release/lct"`；源码仓库也包含完整的 `corpus/` 数据。

## 随包语料：包含什么，不包含什么

这是固定的 **2026092702 中文参考快照**，不是自动追新的在线语料服务。

| 内容 | 覆盖范围 |
| --- | --- |
| 零协会原始发布 ZIP | 2,237 个 JSON 来源文件及原始许可证 |
| 中文字段索引 | 173,300 条文本字段、28,997 条名称字段；不是唯一术语数或翻译完成量 |
| Wiki 参考库 | 35 个采集页面，其中 33 个可检索；285 条名称候选记录，对应 191 个不同名称；191 条段落 |
| 分发辅助文件 | 来源清单、SHA-256 校验、离线恢复脚本、第三方许可及修改说明 |

源数据与元数据中的本机路径已替换为可移植路径。具体版本和来源见 [corpus/manifest.json](corpus/manifest.json)。

**中文参考库不等于已对齐的韩中翻译记忆（TM）。** 当前 WebUI/MCP 的社区参考接口检索 Wiki；零协会原始 JSON 恢复到新工作区的 `references/zh-CN`，其 173,300 条文本字段由独立 MCP 工具 `search_corpus` 直接检索并返回来源文件、JSON Pointer 与哈希；这仍不是韩中自动对齐，WebUI 的 Wiki 搜索不等同于此中文字段搜索。

需要双语 TM 时，另行提供合法取得、版本匹配的韩文 JSON，然后显式索引：

```bash
./lct --data-dir "$HOME/lct-review/data" index \
  --kr /absolute/path/to/matching/KR \
  --zh "$HOME/lct-review/references/zh-CN" \
  --origin zero-association-2026092702 --priority 100
```

不分发私人工作库、个人草稿、审校状态、草稿授权、韩文游戏资源或游戏二进制。快照更新必须显式进行，不在启动时悄悄联网替换。

## 完整分发清单

| 内容 | 仓库 / Linux 发布包 |
| --- | --- |
| CLI、WebUI、MCP | 仓库含源码；发布包内嵌前端，无需安装构建工具 |
| 中文参考快照及 Wiki | `corpus/`，含原始 ZIP、索引、来源、许可、校验与离线恢复 |
| 调查、文风、角色、文学与剧情研究 | [`research/`](research/README.md)，65 份历史报告，分类索引、原始/导出哈希、可信度警示 |
| AI 接入 | 工作区生成器、13 个 MCP 工具、项目 Skill、完整接口文档 |
| 历史语料维护及资源调查工具 | [`tools/`](tools/README.md)，15 个 Python/依赖文件，保留来源哈希与历史适用边界 |
| 发布验证及 UI 图片 | `scripts/`、`docs/`，发布包完整校验清单 `SHA256SUMS` |

**有意不分发**：私人工作库、草稿/审校证据、生产任务与权限文件、账号密钥、个人暂停/交接状态、游戏二进制和韩文原始资源。它们不是其他用户运行工具的前提，不能用复制原用户的生产状态来实现“开箱即用”。历史资料和维护工具的完整分发，也不意味着其结论已经核实或所有历史外部输入均可再分发。

## 使用流程与 MCP

```text
准备匹配版本的原文与参考
          ↓
索引 TM → 提取原文 → 确认任务范围
                         ↓
                草稿 → QA → 人工审校
                         ↓
                   写回 → 审计 / 恢复
```

常用命令：`init`、`index`、`extract`、`search`、`qa`、`fix`、`scope-import`、`scope-status`、`draft-grant`、`draft-revoke`、`inject`、`restore`、`ui`、`mcp`；参数以 `./lct <command> --help` 为准。

智能体使用同一个工作库与显式项目，通过 stdio 连接：

```bash
./lct --data-dir "$HOME/lct-review/data" \
  --reference-db "$HOME/lct-review/references/references.sqlite3" \
  --research-dir "$PWD/research" \
  mcp --project-id review
```

将命令配置到 MCP 客户端时，应使用绝对路径。不传入 `--draft-permit` 的连接只读；写入草稿需要人工事先签发、与任务和原文快照绑定的许可。项目 Skill 位于 `.agents/skills/lct-localization/`，完整接口、授权和回读规则见 [智能体集成说明](docs/AI_AGENT_INTEGRATION.md)。

## 安全与适用范围

- **本机单用户**：HTTP 仅监听回环地址，无远程身份认证；不要通过代理暴露到公网。
- **人工终审**：参考候选、导入译文、AI 草稿和 QA 通过均不等于人工确认。
- **能力边界**：QA 检查不是语义准确性或游戏内显示效果的保证；MCP 写入限制也不等于宿主系统权限隔离。
- **先备份再操作**：首次使用请在独立工作目录验证，注入与恢复前核对目标路径；不要直接用生产文件试验。
- **非官方工具**：不隶属于 Project Moon 或零协会，不将第三方语料重新标为本项目原创内容。

## 开发与发布

```text
crates/       core · parser · tm · qa · server · cli · mcp
ui/           React + Vite 本地审校界面
corpus/     公开参考快照、校验和与独立许可
scripts/      离线恢复、集成验收与发布打包
docs/         接口说明、设计说明、验证边界与 UI 截图
```

在源码仓库中运行：

```bash
cargo test --locked --workspace
pnpm --dir ui format:check
pnpm --dir ui test:editor
python3 scripts/verify-corpus.py "$PWD"
bash scripts/package-release.sh /absolute/output-directory
```

打包会在临时源码目录重新安装锁定依赖、构建前后端，并验证包外启动与真实 HTTP/MCP 参考检索。发布包附带截图、语料、许可证和校验和；进一步验收可运行 `python3 scripts/accept-release.py /absolute/path/to/unpacked/lct`。尚未完成的签收与安全边界见 [验证清单](docs/PRODUCTION_TODO.md)。

## 许可与致谢

- **工具代码**：[MIT](LICENSE) · Antigravity & Hackimi Localization Team。
- **零协会语料**：保留 **CC BY-NC-SA 4.0**，见 [原始许可证](corpus/licenses/ZeroAssociation-LICENSE.txt)。
- **Wiki 内容**：遵循站点默认 **CC BY-SA 4.0** 及页面单独声明，保留贡献者署名、历史链接和第三方权利声明，见 [来源许可记录](corpus/licenses/wiki-source-license.json)。截图中的第三方引用沿用其原许可。

软件的 MIT 许可不覆盖上述第三方内容；使用或再分发语料前，请分别阅读对应许可。
