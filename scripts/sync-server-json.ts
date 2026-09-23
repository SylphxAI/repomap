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
