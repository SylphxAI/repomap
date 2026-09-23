/**
 * Spine SDK — thin isomorphic client over the Rust architecture engine.
 * Same tools as MCP/CLI (`architecture_*`).
 */
import { runTool } from './tools.js';

export type SpineRootOptions = {
  root: string;
};

export class Spine {
  readonly root: string;

  constructor(options: SpineRootOptions) {
    this.root = options.root;
  }

  static create(options: SpineRootOptions): Spine {
    return new Spine(options);
  }

  private call(tool: string, input: Record<string, unknown> = {}) {
    return runTool(tool, { root: this.root, ...input });
  }

  index(input: {
    mode?: 'refresh' | 'auto' | 'full' | 'status_only';
    include?: string[];
    exclude?: string[];
    maxFileBytes?: number;
    useSynth?: boolean;
  } = {}) {
    return this.call('architecture_index', input);
  }

  status() {
    return this.call('architecture_status');
  }

  explain(input: Record<string, unknown> = {}) {
    return this.call('architecture_explain', input);
  }

  overview(input: Record<string, unknown> = {}) {
    return this.call('architecture_overview', input);
  }

  search(query: string, input: Record<string, unknown> = {}) {
    return this.call('architecture_search', { query, ...input });
  }

  path(from: string, to: string, input: Record<string, unknown> = {}) {
    return this.call('architecture_path', { from, to, ...input });
  }

  trace(from: string, to: string, input: Record<string, unknown> = {}) {
    return this.call('architecture_trace', { from, to, ...input });
  }

  impact(changedPaths: string[] | { useGitDiff: true; gitBase?: string }, input: Record<string, unknown> = {}) {
    if (Array.isArray(changedPaths)) {
      return this.call('architecture_impact', { changedPaths, ...input });
    }
    return this.call('architecture_impact', { ...changedPaths, ...input });
  }

  evidence(ids: string[], input: Record<string, unknown> = {}) {
    return this.call('architecture_evidence', { ids, ...input });
  }

  /** Advanced: focus neighborhood + co-located modules + evidence pack */
  contextPack(focus: string, input: Record<string, unknown> = {}) {
    return this.call('architecture_context_pack', { focus, ...input });
  }
}

export default Spine;
