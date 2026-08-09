<!-- Marketing: promise → CTA → comparison → why → docs -->
<div align="center">

# Spine

### Architecture evidence for agents — path, impact, boundaries.

**Local-first repository architecture graphs** with file-level provenance — not dashboard screenshots or keyword dumps.

**Canonical** [`@sylphx/spine`](https://www.npmjs.com/package/@sylphx/spine) · **bin** `spine` · **live** `0.3.1`

[![npm version](https://img.shields.io/npm/v/@sylphx/spine?style=flat-square)](https://www.npmjs.com/package/@sylphx/spine)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](https://opensource.org/licenses/MIT)
[![stars](https://img.shields.io/github/stars/SylphxAI/architecture-reader-mcp?style=flat-square)](https://github.com/SylphxAI/architecture-reader-mcp/stargazers)

</div>

## Zero-config in one line

```bash
npx -y @sylphx/spine
```

No Docker. No cloud code-intel SaaS required for the core path. Stdio MCP for agents.

| Client | Setup |
| --- | --- |
| **Any agent / CLI** | `npx -y @sylphx/spine` |
| **Claude Code** | `claude mcp add spine -- npx -y @sylphx/spine` |
| **Desktop / Cursor / VS Code / Codex** | `"command": "npx", "args": ["-y", "@sylphx/spine"]` |

## Why Spine feels unfairly good

Your agent mapped the repo. **Did it trace the right boundary?**

| Keyword grep map | **Spine** |
| --- | --- |
| “40 files match the string” | **Path / impact / boundaries with provenance** |
| Dashboard screenshots | **Machine-readable architecture evidence** |
| Cloud code-intel by default | **Local-first graph** |
| Setup: SaaS + keys | **`npx -y` — done** |
| Confused with code search | Pairs with **Locus** (chunks) vs **Spine** (system) |

### Five reasons teams pick Spine

1. **Architecture answers**, not file dumps.
2. **Zero-config MCP** — `npx -y @sylphx/spine`.
3. **Local graph** — no required cloud intel product.
4. **Brand-sole** — `@sylphx/spine` / `spine` / `serverInfo.name=spine`.
5. **Family ready** — Locus finds code; Spine maps structure.

## Product docs

| Doc | Purpose |
| --- | --- |
| [docs/POSITIONING.md](docs/POSITIONING.md) | Strategic positioning |
| [docs/COMPETITIVE.md](docs/COMPETITIVE.md) | Peer anchors and wedge |
| [docs/EVIDENCE_CONTRACT.md](docs/EVIDENCE_CONTRACT.md) | Evidence = result contract |
| [docs/TOOL_SURFACE.md](docs/TOOL_SURFACE.md) | Few clear tools policy |
| [docs/PRODUCT_INDEPENDENCE.md](docs/PRODUCT_INDEPENDENCE.md) | This repo is SSOT |
| [docs/IPPB.md](docs/IPPB.md) | Independent public product bar |
| [docs/PUBLISH.md](docs/PUBLISH.md) | npm / git publish status |

## See it work

**Build the Rust engine, then run the MCP adapter locally:**

```bash
git clone https://github.com/SylphxAI/architecture-reader-mcp.git
cd architecture-reader-mcp
bun install
cargo build --release
ARCHITECTURE_READER_CLI=$PWD/target/release/architecture-reader-cli bun run packages/mcp-server/src/index.ts
```

Index once, then ask architecture questions:

```json
{
  "root": "/absolute/path/to/repo",
  "mode": "auto"
}
```

`architecture_search` returns ranked nodes with evidence, not prose:

```json
{
  "status": "ok",
  "repository": {
    "root": "/abs/path",
    "indexedCommit": "abc123",
    "currentCommit": "abc123",
    "freshness": "fresh"
  },
  "answer": {
    "matches": [
      {
        "id": "cmp_auth",
        "kind": "boundary",
        "label": "Authentication",
        "score": 0.94
      }
    ]
  },
  "evidence": [
    {
      "id": "ev_01",
      "kind": "ast",
      "path": "src/auth/middleware.ts",
      "startLine": 10,
      "endLine": 42,
      "extractor": "synth-typescript@0.3.x",
      "confidence": "deterministic"
    }
  ],
  "gaps": [],
  "metrics": { "elapsedMs": 12, "nodeCount": 430, "edgeCount": 910 }
}
```

Abbreviated shape — full envelope in [tool contract spec](./docs/specs/2026-07-09-tool-contract.md).

Trace dependency or impact before editing:

```json
{
  "from": "src/auth/middleware.ts",
  "to": "src/billing/webhook.ts",
  "relation": "depends_on"
}
```

## Why agents will use it

| Need | Tool |
| --- | --- |
| Build or refresh the index | `architecture_index` |
| Check freshness and coverage | `architecture_status` |
| Top-level repo map | `architecture_overview` |
| Find boundaries, routes, schemas | `architecture_path` | Shortest path with hop provenance (`extracted`/`inferred`) |
| `architecture_search` |
| Follow dependency or call paths | `architecture_trace` |
| Estimate diff blast radius (incoming dependents + outgoing deps) | `architecture_impact` |
| Local neighborhood (Graphify-class) | `architecture_overview` focus=… or `architecture_search` `includeNeighbors:true` |
| Short **import** cycles | `architecture_overview` → `cycles` (imports/depends only) |
| Top fan-in / fan-out modules | `architecture_overview` / `status` → `topFanIn` / `topFanOut` |
| Browse important nodes | `architecture_search` with empty query (fan-in rank) |
| Relation kind histogram | `architecture_status` → `relationKinds` |
| Unknown changed paths | `architecture_impact` → `unknownImpact` |
| Unresolved path ends | `architecture_path` → `suggestions` |
| Impact edge labels | `fromNode` / `toNode` summaries on impact edges |
| Fetch proof behind a claim | `architecture_evidence` |

Every answer shares one evidence envelope: path, optional line range, extraction
source, freshness, confidence, and known gaps.

## Repository layout

```text
architecture-reader-mcp/
  crates/
    architecture-reader-core/    # Rust architecture graph contracts and engine
  packages/
    mcp-server/                  # TypeScript/Bun MCP adapter
  docs/
    adr/                         # Architecture decisions
    specs/                       # Product, graph, indexing, and tool specs
    research/                    # Evidence and category analysis
    portfolio/                   # MCP portfolio ADRs and roadmaps
  server.json                    # MCP server metadata
```

## Design documents

| Topic | Link |
| --- | --- |
| Architecture overview | [docs/architecture.md](./docs/architecture.md) |
| Product spec | [docs/specs/2026-07-09-product-spec.md](./docs/specs/2026-07-09-product-spec.md) |
| Tool contract | [docs/specs/2026-07-09-tool-contract.md](./docs/specs/2026-07-09-tool-contract.md) |
| Evidence graph | [docs/specs/2026-07-09-evidence-graph.md](./docs/specs/2026-07-09-evidence-graph.md) |
| Indexing pipeline | [docs/specs/2026-07-09-indexing-pipeline.md](./docs/specs/2026-07-09-indexing-pipeline.md) |
| Category research | [docs/research/2026-07-09-category-and-internal-analysis.md](./docs/research/2026-07-09-category-and-internal-analysis.md) |
| Portfolio plan | [docs/portfolio/README.md](./docs/portfolio/README.md) |
| Roadmap | [docs/portfolio/roadmaps/architecture-reader-mcp.md](./docs/portfolio/roadmaps/architecture-reader-mcp.md) |

## Agent skill surface

Codex/Claude-style skill: [`skills/spine/SKILL.md`](./skills/spine/SKILL.md) — install, tools, evidence contract (Graphify-class agent UX without multi-GB weight).

## Tool contract

All seven `architecture_*` tools share the evidence envelope defined in
[tool contract spec](./docs/specs/2026-07-09-tool-contract.md). Run
`bun run benchmark:release-gate` after `cargo build --release` for boundary proof.

## Development

```bash
bun install
bun run validate
cargo build --release
cargo test
bun test
bun run benchmark:public-proof
```

## Security model

- **Repository scope** — all tools operate relative to the configured project root; absolute paths are rejected.
- **Evidence envelope** — every answer includes path, line range, extractor route, freshness, and explicit coverage gaps.
- **Deterministic first** — regex and manifest extractors are labeled; Synth AST is opt-in via `ARCHITECTURE_READER_USE_SYNTH=1`.
- **Local-first** — indexing and queries run on your machine; no document upload to Sylphx cloud by default.

## Benchmark proof

Reproduce locally:

```bash
bun run build:rust
bun run benchmark:public-proof
bun run benchmark:release-gate
```

Fixture: `fixtures/sample-repo` (auth middleware + ADR + package manifest). Example requests: [`examples/`](examples/).

## Help this reach more builders

If your agent has ever refactored the wrong module because it guessed the architecture,
this project is for you.

**[⭐ Star the repo](https://github.com/SylphxAI/architecture-reader-mcp)** — it helps
more agent builders find evidence-backed architecture answers before irreversible edits.

### Discovery (in progress)

| Channel | Status |
| --- | --- |
| [Official MCP Registry](https://registry.modelcontextprotocol.io/) | Not listed yet — Beta 0.1 local ship, no publish workflow |
| [Glama MCP directory](https://glama.ai/mcp/servers) | Not listed yet |
| [mcpservers.org submit](https://mcpservers.org/submit) | Not listed yet — free web-form submission |
| [mcp.so submit](https://mcp.so/submit) | Not listed yet — directory submission |

Know another MCP directory? [Open an issue](https://github.com/SylphxAI/architecture-reader-mcp/issues/new) with the link.


## Spine CLI / SDK

```bash
bun run build:rust
./bin/spine index .
./bin/spine search . auth
./bin/spine path . authMiddleware validateToken --relation calls
```

TypeScript:
```ts
import { Spine } from '@sylphx/spine/sdk'
const spine = Spine.create({ root: process.cwd() })
await spine.index({ mode: 'full' })
const path = await spine.path('authMiddleware', 'validateToken', { relation: 'calls' })
```

## License

MIT — see [LICENSE](LICENSE).
