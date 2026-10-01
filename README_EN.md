# LCT · Limbus Company Translate

简体中文 | **[English]**

An industrial-grade KR → ZH-CN localization engineering platform for Limbus Company: a Rust workspace centered on a translation-memory core, with a CLI pipeline, a local WebUI dashboard, and an MCP interface for AI agents.

> The boundary of language is not the boundary of understanding.

## What it is

LCT treats game-text localization as a gated engineering pipeline, not a one-off script:

- **Index** bilingual reference corpora into a SQLite FTS5 full-text translation memory (TM)
- **Extract** incremental diffs between raw source and current translations into work orders
- **Inject** translations back in place via RFC 6901 JSON Pointers — lossless, untouched fields stay untouched
- **Gate** every translation unit with L-level QA rules (tildes, placeholders, spacing, terminology consistency…); AI drafts must pass the gate to enter the database and are never marked as "human-reviewed"
- **Review** happens in a local WebUI: bilingual workbench, corpus search, provenance & audit, focused review queue

The toolchain is AI-first (the CLI performs authorized indexing/extraction/file operations; MCP offers project reads, QA, and draft submission), while the WebUI is where humans observe, verify, and approve.

## Three interfaces

### 1. CLI

```bash
lct init                       # initialize a workspace
lct index                      # index bilingual corpora, feed the TM
lct extract                    # extract incremental untranslated segments (JSONL work orders)
lct inject                     # safely inject back into game JSON
lct qa                         # localization gates & semantic-consistency audit
lct fix                        # deterministic safe auto-fixes
lct search <term>              # millisecond full-text search over real in-game lines
lct scope-import / scope-status   # validate / read task scope
lct draft-grant / draft-revoke    # issue / revoke AI draft authorization
lct restore                    # explicit backup restore, no unrelated overwrites
lct ui                         # launch the local WebUI dashboard
lct mcp                        # serve bounded project tools over MCP stdio
```

### 2. WebUI (local dashboard)

```bash
lct ui --project-id review --port 3000
```

Listens on loopback only. Five surfaces: overview (observation instrument), bilingual workbench, corpus search, provenance & audit, focused review. The UI is embedded into the single binary via rust-embed — no static assets to ship.

### 3. MCP (AI agents)

```bash
lct mcp --project-id review
```

For an explicit local single-user project: AI can read the project, run QA, and submit drafts — drafts require a human-issued `draft-grant` beforehand, and still land in the review queue where a human makes the final call.

## Build

Frontend first; release builds embed `ui/dist` into the binary:

```bash
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui build
cargo build --release --locked -p loc-cli
./target/release/lct ui --no-open
```

Requires Rust 1.89+, Node.js + pnpm. Target: local single-user Linux x86_64 workspaces.

## Repository layout

```
crates/
  loc-core     core models, SQLite storage, audit, terminology
  loc-parser   AST extractor / RFC 6901 injector / asset categories
  loc-tm       translation-memory engine
  loc-qa       QA gates and auto-fixes
  loc-server   local HTTP API with embedded assets
  loc-cli      the lct command line
  loc-mcp      MCP stdio server
ui/            React + Vite dashboard (Rhine Lab art direction)
docs/          interface guide, production checklist, UI art direction
scripts/       release and verification scripts
```

## Design red lines

- **Local, single user**: no remote auth, no multi-user project selection, no remote MCP — do not expose the service through a proxy
- **Human final review**: AI drafts always enter the review queue; passing QA gates is not human sign-off
- **Lossless write-back**: injection never touches unrelated fields; restore only reverts what the backup covers
- **Reduced motion**: every UI animation honors both the OS `prefers-reduced-motion` setting and the in-app toggle

See [docs/AI_AGENT_INTEGRATION.md](docs/AI_AGENT_INTEGRATION.md) for interfaces and security boundaries, and [docs/UI_ART_DIRECTION.md](docs/UI_ART_DIRECTION.md) for the art direction.

## License

[MIT](LICENSE) · Antigravity & Hackimi Localization Team
