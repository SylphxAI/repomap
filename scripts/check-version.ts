// Fails when any manifest disagrees with packages/repomap/package.json.
import { readFileSync, readdirSync } from "node:fs";

const read = (p: string) => JSON.parse(readFileSync(p, "utf8"));
const v = read("packages/repomap/package.json").version;
const bad: string[] = [];
const eq = (what: string, got: string) => got !== v && bad.push(`${what}: ${got}`);
for (const [k, x] of Object.entries(read("packages/repomap/package.json").optionalDependencies)) eq(k, x as string);
for (const d of readdirSync("packages/npm")) eq(`packages/npm/${d}`, read(`packages/npm/${d}/package.json`).version);
for (const d of readdirSync("packages/aliases")) {
  const p = read(`packages/aliases/${d}/package.json`);
  eq(`packages/aliases/${d}`, p.version);
  eq(`packages/aliases/${d} dep`, p.dependencies["@sylphx/repomap"]);
}
const s = read("server.json");
eq("server.json", s.version);
eq("server.json package", s.packages[0].version);
const cargo = readFileSync("Cargo.toml", "utf8").match(/\[workspace\.package\][^\[]*?version = "([^"]+)"/)?.[1] ?? "";
eq("Cargo.toml", cargo);
if (bad.length) {
  console.error(`version mismatch (want ${v}):\n  ${bad.join("\n  ")}`);
  process.exit(1);
}
console.log(`all manifests at ${v}`);
