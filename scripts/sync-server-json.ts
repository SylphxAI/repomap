import { readFileSync, writeFileSync } from 'node:fs';

type Pkg = {
  version: string;
  optionalDependencies?: Record<string, string>;
};

const pkgPath = 'packages/mcp-server/package.json';
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8')) as Pkg;

// The platform native packages are versioned together with the product. When the
// version PR bumps the product, the optionalDependencies must move with it — or
// npm keeps installing the previous native binary and a source fix never reaches
// a user. Keep them pinned to the product version here, where the bump happens.
if (pkg.optionalDependencies) {
  for (const name of Object.keys(pkg.optionalDependencies)) {
    pkg.optionalDependencies[name] = pkg.version;
  }
  writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);
}

const server = JSON.parse(readFileSync('server.json', 'utf8')) as {
  version: string;
  packages: Array<{ version: string }>;
};

server.version = pkg.version;
server.packages[0].version = pkg.version;

writeFileSync('server.json', `${JSON.stringify(server, null, 2)}\n`);

// The root package.json is a private workspace container, but its `version`
// and `mcpName` are what a reader and the registry tooling see first. Keep the
// version aligned with the publishable package so the repo root never reports a
// version that has no release behind it.
const rootPath = 'package.json';
const root = JSON.parse(readFileSync(rootPath, 'utf8')) as { version?: string };
if (root.version !== undefined && root.version !== pkg.version) {
  root.version = pkg.version;
  writeFileSync(rootPath, `${JSON.stringify(root, null, 2)}\n`);
}

// Keep the Rust MCP server's advertised version aligned with the product
// version, so serverInfo never reports a stale release.
const rustLib = 'crates/architecture-reader-mcp-server/src/lib.rs';
try {
  const raw = readFileSync(rustLib, 'utf8');
  const next = raw.replace(
    /(pub const SERVER_VERSION: &str = ")[^"]*(";)/,
    `$1${pkg.version}$2`,
  ).replace(
    /(None => ")[0-9][^"]*(")/,
    `$1${pkg.version}$2`,
  );
  if (next !== raw) writeFileSync(rustLib, next);
} catch {
  // Rust source is not present in every checkout.
}
