# MCP tools

Every tool is read-only and takes optional `root` (repository path) and `format` (`text`, the default, or `json`).

## `map`

Start here. Returns modules (communities of files that depend on each other), the most central files, the most used symbols and entry points.

| Arg | Type | |
|---|---|---|
| `focus` | string | Directory to zoom into. Adds an outline of files and symbols with line numbers. |
| `limit` | integer | Items per section (default 12) |

## `search`

Hybrid search that fuses symbol-name matches with BM25 over AST chunks, using reciprocal-rank fusion.

| Arg | Type | |
|---|---|---|
| `query` | string, required | Words or identifiers, e.g. `refresh token expiry`, `parseConfig` |
| `limit` | integer | Default 10 |
| `path` | string | Only files whose path starts with or contains this |
| `kind` | enum | `function`, `method`, `class`, `struct`, `interface`, `trait`, `enum`, `type`, `module`, `macro` |
| `include_tests` | boolean | Default true (tests rank lower) |

## `context`

A 360° view of one symbol or file.

| Arg | Type | |
|---|---|---|
| `target` | string, required | `path/to/file.ts`, `file.ts:42` (the symbol at that line), `Class.method`, `Class::method`, or a name |
| `code_lines` | integer | Source lines to include for a symbol (default 60, 0 for none) |

For a symbol it returns the code, callers with call-site lines, callees, subtypes and implementations, members, and the tests that reach it within three calls. For a file it returns the outline, imports, importers, callers from other files and tests.

## `trace`

| Arg | Type | |
|---|---|---|
| `from` | string, required | Symbol or file |
| `to` | string | With `to`: the shortest call path, which falls back to the reverse direction, then to the file dependency path |
| `direction` | `callees` \| `callers` | Without `to`: the call tree below or above `from` |
| `depth` | integer | Tree depth (default 3) |

## `impact`

| Arg | Type | |
|---|---|---|
| `target` | string or string[] | Symbols or files to change |
| `changed` | boolean | Use the working-tree `git diff` (and untracked files) instead. Changed lines are mapped to the symbols that contain them. |
| `base` | string | Git ref for `changed` (default `HEAD`), e.g. `main` |
| `depth` | integer | Caller depth (default 3) |

Returns a risk level (low, medium or high), callers grouped by depth with call sites, the files that import the target files, the modules touched and the test files to run.

## Legacy names

`architecture_*`, `codebase_search` and `find_related` are accepted until 2.0. See the [migration guide](/guide/migrate).
