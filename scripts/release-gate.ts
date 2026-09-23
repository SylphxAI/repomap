import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { invokeEngine } from '../packages/mcp-server/src/engine.ts';
import { runDoctor } from '../packages/mcp-server/src/doctor.ts';

const ARTIFACT_DIR_ENV = 'MCP_ARCHITECTURE_BENCHMARK_OUTPUT_DIR';
const DEFAULT_ARTIFACT_DIR = 'benchmark-artifacts';
const ARTIFACT_FILE = 'architecture_reader_release_gate.json';

type GateStatus = 'passed' | 'failed';

interface GateCheck {
  id: string;
  status: GateStatus;
  message: string;
  evidence?: Record<string, unknown>;
}

interface ReleaseGateReport {
  profile: 'architecture_reader_release_gate';
  generated_at: string;
  artifact_dir: string;
  status: GateStatus;
  summary: {
    total: number;
    passed: number;
    failed: number;
  };
  checks: GateCheck[];
}

const repoRoot = path.resolve(import.meta.dirname, '..');
const fixtureRoot = path.join(repoRoot, 'fixtures/sample-repo');

const addCheck = (
  checks: GateCheck[],
  id: string,
  passed: boolean,
  message: string,
  evidence?: Record<string, unknown>
): void => {
  checks.push({
    id,
    status: passed ? 'passed' : 'failed',
    message,
    ...(evidence ? { evidence } : {}),
  });
};

const fileExists = (relativePath: string): boolean =>
  existsSync(path.join(repoRoot, relativePath));

export async function buildReleaseGateReport(artifactDir: string): Promise<ReleaseGateReport> {
  const checks: GateCheck[] = [];
  const pkg = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')) as {
    version: string;
  };

  addCheck(
    checks,
    'rust:graph_core',
    fileExists('crates/architecture-reader-core/src/engine.rs'),
    'Rust architecture-reader-core graph engine is present'
  );

  addCheck(
    checks,
    'fixtures:sample_repo',
    fileExists('fixtures/sample-repo/package.json'),
    'Golden sample-repo fixture is checked in'
  );

  addCheck(
    checks,
    'spec:indexing_pipeline',
    fileExists('docs/specs/2026-07-09-indexing-pipeline.md'),
    'Indexing pipeline spec documents incremental refresh behavior'
  );

  const doctor = await runDoctor(pkg.version);
  addCheck(
    checks,
    'doctor:fixture_repo',
    doctor.checks.find((check) => check.id === 'fixture_repo')?.status === 'ok',
    'doctor reports the golden fixture repository is available',
    { doctorStatus: doctor.status }
  );

  const index = invokeEngine('architecture_index', { root: fixtureRoot, mode: 'full' });
  addCheck(
    checks,
    'boundary:architecture_index',
    index.status === 'ok' && (index.metrics?.nodeCount ?? 0) > 0,
    'architecture_index returns a populated graph envelope from the Rust CLI',
    { nodeCount: index.metrics?.nodeCount, edgeCount: index.metrics?.edgeCount }
  );

  const search = invokeEngine('architecture_search', {
    root: fixtureRoot,
    query: 'auth',
    limit: 5,
  });
  const matches = (search.answer as { matches?: unknown[] } | undefined)?.matches ?? [];
  addCheck(
    checks,
    'boundary:architecture_search',
    search.status === 'ok' && matches.length > 0,
    'architecture_search returns stable evidence locators for the fixture repo',
    { matchCount: matches.length }
  );

  const auto = invokeEngine('architecture_index', { root: fixtureRoot, mode: 'auto' });
  addCheck(
    checks,
    'boundary:incremental_cache_hit',
    auto.status === 'ok' &&
      (auto.answer as { refreshMode?: string } | undefined)?.refreshMode === 'cache_hit',
    'architecture_index mode=auto reuses the persisted index when file hashes are unchanged',
    { refreshMode: (auto.answer as { refreshMode?: string } | undefined)?.refreshMode }
  );

  const trace = invokeEngine('architecture_trace', {
    root: fixtureRoot,
    from: 'src/server/routes.ts',
    to: '../auth/middleware.js',
    relation: 'imports',
    maxDepth: 6,
  });
  const tracePath = (trace.answer as { path?: unknown[] } | undefined)?.path ?? [];
  addCheck(
    checks,
    'boundary:architecture_trace',
    trace.status === 'ok' && tracePath.length > 0,
    'architecture_trace returns a non-empty path between routes and auth modules in the fixture repo',
    { pathLength: tracePath.length }
  );

  const callTrace = invokeEngine('architecture_trace', {
    root: fixtureRoot,
    from: 'authMiddleware',
    to: 'validateToken',
    relation: 'calls',
    maxDepth: 4,
  });
  const callTracePath = (callTrace.answer as { path?: unknown[] } | undefined)?.path ?? [];
  addCheck(
    checks,
    'boundary:architecture_symbol_call_trace',
    callTrace.status === 'ok' && callTracePath.length >= 2,
    'architecture_trace follows symbol call edges between authMiddleware and validateToken',
    { pathLength: callTracePath.length }
  );

  const pythonIndex = invokeEngine('architecture_index', { root: fixtureRoot, mode: 'full' });
  const graphPath = path.join(fixtureRoot, '.architecture-reader', 'graph.json');
  const graph = existsSync(graphPath)
    ? (JSON.parse(readFileSync(graphPath, 'utf8')) as {
        nodes?: Array<{ kind?: string; path?: string }>;
        edges?: Array<{ kind?: string; from?: string; to?: string }>;
      })
    : {};
  const hasPythonModule = (graph.nodes ?? []).some(
    (node) => node.kind === 'module' && node.path === 'src/ml/scorer.py'
  );
  const hasPythonImport = (graph.edges ?? []).some(
    (edge) => edge.kind === 'imports' && edge.from?.includes('scorer.py')
  );
  addCheck(
    checks,
    'boundary:python_adapter',
    pythonIndex.status === 'ok' && hasPythonModule && hasPythonImport,
    'Python files are indexed with import edges in the architecture graph',
    { hasPythonModule, hasPythonImport }
  );

  const impact = invokeEngine('architecture_impact', {
    root: fixtureRoot,
    changedPaths: ['src/auth/token.ts'],
    maxDepth: 2,
  });
  const impactAnswer = impact.answer as {
    directImpact?: unknown[];
    incomingImpact?: unknown[];
    outgoingImpact?: unknown[];
    maxDepth?: number;
  } | undefined;
  const directImpact = impactAnswer?.directImpact ?? [];
  const incomingImpact = impactAnswer?.incomingImpact ?? [];
  const outgoingImpact = impactAnswer?.outgoingImpact ?? [];
  addCheck(
    checks,
    'boundary:architecture_impact',
    impact.status === 'ok' && directImpact.length > 0,
    'architecture_impact reports direct impact nodes for changed fixture paths',
    { directImpactCount: directImpact.length }
  );
  addCheck(
    checks,
    'boundary:architecture_impact_reverse',
    impact.status === 'ok' && (incomingImpact.length > 0 || outgoingImpact.length > 0),
    'architecture_impact reports reverse (incoming) or outgoing blast-radius edges',
    {
      incoming: incomingImpact.length,
      outgoing: outgoingImpact.length,
      maxDepth: impactAnswer?.maxDepth,
    }
  );

  const pathProbe = invokeEngine('architecture_path', {
    root: fixtureRoot,
    from: 'src/auth/token.ts',
    to: 'src/server/routes.ts',
    maxDepth: 8,
  });
  const hopCount = (pathProbe.answer as { hopCount?: number } | undefined)?.hopCount ?? 0;
  const overviewFocus = invokeEngine('architecture_overview', {
    root: fixtureRoot,
    focus: 'src/auth/token.ts',
    depth: 3,
  });
  const neighbors =
    (overviewFocus.answer as { neighbors?: unknown[] } | undefined)?.neighbors ?? [];
  addCheck(
    checks,
    'boundary:architecture_overview_neighbors',
    overviewFocus.status === 'ok' && neighbors.length > 0,
    'architecture_overview focus returns Graphify-class neighbors for a fixture path',
    { neighborCount: neighbors.length }
  );

  addCheck(
    checks,
    'boundary:architecture_path',
    pathProbe.status === 'ok',
    'architecture_path returns hop provenance envelope (empty path allowed with gaps)',
    { hopCount, status: pathProbe.status }
  );

  addCheck(
    checks,
    'surface:agent_skill',
    fileExists('skills/spine/SKILL.md'),
    'Graphify-class agent skill surface is present at skills/spine/SKILL.md'
  );

  addCheck(
    checks,
    'docs:brand_publish',
    fileExists('docs/BRAND_PUBLISH.md'),
    'Brand publish readiness doc is present'
  );

  addCheck(
    checks,
    'brand:server_json_title_spine',
    (JSON.parse(readFileSync(path.join(repoRoot, 'server.json'), 'utf8')) as { title?: string })
      .title === 'Spine',
    'server.json marketplace title is Spine'
  );

  const hasRust = (graph.nodes ?? []).some(
    (node) => node.kind === 'module' && (node.path ?? '').endsWith('.rs'),
  );
  const hasGo = (graph.nodes ?? []).some(
    (node) => node.kind === 'module' && (node.path ?? '').endsWith('.go'),
  );
  const hasJava = (graph.nodes ?? []).some(
    (node) => node.kind === 'module' && (node.path ?? '').endsWith('.java'),
  );
  addCheck(
    checks,
    'boundary:multi_language_modules',
    hasRust && hasGo && hasJava && hasPythonModule,
    'Fixture indexes Rust, Go, Java, and Python modules',
    { hasRust, hasGo, hasJava, hasPythonModule }
  );

  const matrixProbe = spawnSync(
    'bun',
    ['test', 'packages/mcp-server/test/shippedPath.matrix.test.ts', '--concurrency', '1'],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      env: {
        ...process.env,
        ARCHITECTURE_READER_ALLOW_LEGACY_ENGINE: '',
        ARCHITECTURE_READER_USE_SYNTH: '',
      },
      timeout: 300_000,
    }
  );
  addCheck(
    checks,
    'boundary:rust_cli_engine',
    fileExists('packages/mcp-server/test/shippedPath.matrix.test.ts') &&
      matrixProbe.status === 0,
    'Shipped-path matrix test proves all eight primary tools route through Rust core without legacy runtime',
    matrixProbe.status === 0
      ? { exitCode: 0 }
      : {
          exitCode: matrixProbe.status,
          stderr: matrixProbe.stderr?.slice(-2000),
          stdout: matrixProbe.stdout?.slice(-2000),
        }
  );

  const goldenParityProbe = spawnSync(
    'cargo',
    ['test', '-p', 'architecture-reader-mcp-server', '--test', 'golden_parity'],
    {
      cwd: repoRoot,
      encoding: 'utf8',
      timeout: 300_000,
    }
  );
  addCheck(
    checks,
    'mcp:golden_parity',
    fileExists('crates/architecture-reader-mcp-server/tests/golden_parity.rs') &&
      goldenParityProbe.status === 0,
    'Golden parity test proves the in-process rmcp adapter envelopes match direct architecture-reader-cli on fixture repo',
    goldenParityProbe.status === 0
      ? { exitCode: 0 }
      : {
          exitCode: goldenParityProbe.status,
          stderr: goldenParityProbe.stderr?.slice(-2000),
          stdout: goldenParityProbe.stdout?.slice(-2000),
        }
  );

  addCheck(
    checks,
    'publish:changeset_config',
    fileExists('.changeset/config.json'),
    'Changesets config is present for npm publish workflow'
  );

  addCheck(
    checks,
    'publish:release_workflow',
    fileExists('.github/workflows/release.yml') &&
      fileExists('.github/workflows/publish-mcp-registry.yml'),
    'Release and MCP Registry publish workflows are present'
  );

  const server = existsSync(path.join(repoRoot, 'server.json'))
    ? (JSON.parse(readFileSync(path.join(repoRoot, 'server.json'), 'utf8')) as {
        packages?: Array<{ identifier?: string }>;
      })
    : {};
  addCheck(
    checks,
    'publish:registry_metadata',
    server.packages?.[0]?.identifier === '@sylphx/spine',
    'server.json documents the publishable npm package identifier'
  );

  const runtimeNote = readFileSync(
    path.join(repoRoot, 'docs/portfolio/notes/rust-first-runtime-distribution.md'),
    'utf8',
  );
  const toolSpec = readFileSync(
    path.join(repoRoot, 'docs/specs/agent-native-tool-surface.md'),
    'utf8',
  );
  addCheck(
    checks,
    'spec:runtime_and_tool_surface',
    runtimeNote.includes('rmcp') &&
      runtimeNote.toLowerCase().includes('rust') &&
      toolSpec.includes('architecture_index') &&
      toolSpec.includes('architecture_evidence'),
    'Rust-first runtime note and agent-native tool spec document the shipped stack and tool surface'
  );

  const passed = checks.filter((check) => check.status === 'passed').length;
  const failed = checks.length - passed;

  return {
    profile: 'architecture_reader_release_gate',
    generated_at: new Date().toISOString(),
    artifact_dir: artifactDir,
    status: failed === 0 ? 'passed' : 'failed',
    summary: {
      total: checks.length,
      passed,
      failed,
    },
    checks,
  };
}

async function main(): Promise<void> {
  const artifactDir = path.resolve(
    process.env[ARTIFACT_DIR_ENV] ?? path.join(repoRoot, DEFAULT_ARTIFACT_DIR)
  );

  const report = await buildReleaseGateReport(artifactDir);
  mkdirSync(artifactDir, { recursive: true });
  const outputPath = path.join(artifactDir, ARTIFACT_FILE);
  writeFileSync(outputPath, `${JSON.stringify(report, null, 2)}\n`, 'utf8');
  console.error(`Architecture reader release gate report written to ${outputPath}`);

  if (report.status !== 'passed') {
    for (const check of report.checks.filter((entry) => entry.status === 'failed')) {
      console.error(`[FAILED] ${check.id}: ${check.message}`);
    }
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((error: unknown) => {
    console.error(error);
    process.exit(1);
  });
}
