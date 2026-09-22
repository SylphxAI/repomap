# Portfolio Note: Shared Agent Evidence Envelope

Status: planning note
Date: 2026-07-09  
Decision owner: SylphxAI

## Context

The portfolio includes readers, code search, architecture tracing, filesystem
operations, and consultation tools. Agents need a consistent way to interpret
results across these products. Raw markdown or free-form text is not enough for
trusted autonomous work.

## Decision

Every MCP should return a structured evidence envelope whenever the result makes
a claim about a file, codebase, document, image, video, command, or decision.

The envelope should include these fields where applicable:

- `subject`: what was inspected or decided;
- `source`: path, URI, package, repository, or provider source;
- `sourceHash`: stable content hash when available;
- `freshness`: timestamp, git commit, index version, or cache status;
- `locator`: line range, byte offset, page, bounding box, timestamp, symbol id,
  node id, operation id, or consultation id;
- `route`: extraction, search, parser, model, or policy path used;
- `confidence`: bounded score or categorical confidence;
- `warnings`: coverage, safety, staleness, unsupported feature, redaction, or
  degraded-mode warnings;
- `nextActions`: follow-up MCP calls that can verify or deepen the result.

The exact schema may vary by project, but the semantics must stay consistent.

## Rationale

Agents fail when they receive confident text without provenance. A shared
evidence envelope turns each MCP into an inspection product. It allows agents
to cite, re-check, compare, cache, and escalate results without guessing where a
claim came from.

## Consequences

Projects must avoid tool outputs that mix narrative, hidden assumptions, and
source facts in one unstructured blob. Human-readable summaries are allowed, but
they must sit next to structured evidence.

Adapters must preserve the evidence envelope exactly. If an underlying provider
or parser cannot return precise evidence, the MCP must say so with a warning
instead of fabricating locators.

## Validation Gates

- Golden fixtures assert stable evidence envelope shape.
- Missing evidence creates a warning, not silent omission.
- Cached outputs expose cache freshness.
- Multi-source outputs keep per-source locators.
- Documentation shows one minimal and one rich JSON example per tool family.
