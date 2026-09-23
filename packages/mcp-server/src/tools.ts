import { z } from 'zod';
import { invokeEngine } from './engine.js';

const rootSchema = z.object({
  root: z.string().describe('Absolute path to the repository root'),
});

export const architectureIndexSchema = rootSchema.extend({
  mode: z
    .enum(['refresh', 'auto', 'full', 'status_only'])
    .optional()
    .describe(
      'Defaults to refresh: cache hit, incremental, or full as needed. auto is the old name for that same behavior. full forces a complete rescan. status_only checks freshness without indexing.',
    ),
  include: z.array(z.string()).optional(),
  exclude: z.array(z.string()).optional(),
  maxFileBytes: z.number().int().positive().optional(),
});

export const architectureStatusSchema = rootSchema;

export const architectureOverviewSchema = rootSchema.extend({
  scope: z.string().optional(),
  depth: z.number().int().positive().optional(),
  focus: z.enum(['runtime', 'data', 'api', 'package', 'delivery', 'docs', 'all']).optional(),
});

export const architectureSearchSchema = rootSchema.extend({
  query: z.string(),
  types: z.array(z.string()).optional(),
  limit: z.number().int().positive().optional(),
  includeEvidence: z.boolean().optional(),
});

export const architecturePathSchema = rootSchema.extend({
  from: z.string().optional(),
  to: z.string().optional(),
  source: z.string().optional(),
  target: z.string().optional(),
  relation: z.string().optional(),
  maxDepth: z.number().int().positive().optional(),
});

export const architectureTraceSchema = rootSchema.extend({
  from: z.string(),
  to: z.string(),
  relation: z.string().optional(),
  maxDepth: z.number().int().positive().optional(),
});

export const architectureImpactSchema = rootSchema.extend({
  changedPaths: z.array(z.string()).optional(),
  useGitDiff: z.boolean().optional(),
  gitBase: z.string().optional(),
  includeTests: z.boolean().optional(),
  includeDocs: z.boolean().optional(),
});

export const architectureEvidenceSchema = rootSchema.extend({
  ids: z.array(z.string()),
  maxBytes: z.number().int().positive().optional(),
});

export function runTool(tool: string, input: Record<string, unknown>) {
  return invokeEngine(tool, input);
}