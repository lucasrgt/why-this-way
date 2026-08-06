import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import test from "node:test";

const ADMIN = ["init", "collect", "supersede", "export"];

const integration = resolve(import.meta.dirname, "..");
const repository = resolve(integration, "../..");

test("package is curated, versioned with Cargo, and has no administrative surface", () => {
  const manifest = JSON.parse(readFileSync(join(integration, "package.json"), "utf8"));
  const cargo = readFileSync(join(repository, "Cargo.toml"), "utf8").match(/^version = "([^"]+)"/m)?.[1];
  const source = readFileSync(join(integration, "src/index.ts"), "utf8");
  assert.equal(manifest.version, cargo);
  assert.deepEqual(manifest.pi.extensions, ["./src/index.ts"]);
  assert.deepEqual(manifest.files, ["src", "skills", "README.md"]);
  for (const command of ADMIN) assert.doesNotMatch(source, new RegExp(`operation === ["']${command}["']`));
  assert.match(source, /csm\.toml/);
});
