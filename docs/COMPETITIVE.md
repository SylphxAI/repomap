# Spine — where it sits

## Job

Repository architecture with file-level proof. An agent asks what a repository is, where a behavior lives, how a request moves, and what a change affects. The answer carries a path and a line.

## What Spine is

A local architecture graph: `architecture_index`, `architecture_search`, `architecture_path`, `architecture_trace`, `architecture_impact`, and `architecture_evidence`. The default index reuses a current graph, updates what changed, or scans fully when it has to. Nothing on that path calls a model or a hosted code-intel API.

## Peers

These tools solve nearby jobs. Spine does not replace them, and this page does not claim a measured win over them.

| Peer | What it is | What Spine does differently |
| --- | --- | --- |
| [Serena](https://github.com/oraios/serena) | A coding agent toolkit. Semantic retrieval and editing, often through language servers. | Spine does not edit files. It returns path, trace, impact, and file:line evidence from a local graph. |
| [ripgrep](https://github.com/BurntSushi/ripgrep) | Fast literal search across file contents. | Spine is not a line search. It indexes boundaries and relations, then answers path, trace, and impact. |
| [Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) | An interactive knowledge graph. A multi-agent pipeline uses a model to build it; the saved graph can be opened later without one. | Spine's default path never calls a model. The result is path, trace, impact, and file:line evidence, not a guided tour. |
| Language servers | Definitions, references, and types for one language, inside an editor. | Spine is a repository graph for an agent: cross-file path, trace, impact, freshness, and gaps. |

## Not the job

- A cloud code-intel account
- A generated summary standing in for evidence
- Code-chunk retrieval (that job belongs to Locus)
- Editing the repository

## Install

```bash
npx -y @sylphx/spine
```

MCP `io.github.SylphxAI/spine`. Docs <https://sylphxai.github.io/spine/>.
