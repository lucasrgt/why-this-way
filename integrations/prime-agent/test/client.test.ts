import assert from "node:assert/strict";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { mkdtempSync } from "node:fs";
import test from "node:test";
import { ToolClient, discoverStandalone } from "../src/client.js";
import type { Exec, ExecResult } from "../src/types.js";

function harness(results: Array<Partial<ExecResult>>) {
  const calls: Array<{ command: string; args: string[]; options: unknown }> = [];
  const exec: Exec = async (command, args, options) => {
    calls.push({ command, args, options });
    const result = results.shift() ?? {};
    return { stdout: "", stderr: "", code: 0, killed: false, ...result };
  };
  return { exec, calls };
}

test("discovery is passive, rejects bad Git output, and gives csm.toml precedence", async () => {
  const root = mkdtempSync(join(tmpdir(), "wtw-prime-"));
  const fake = harness([
    { code: 0, killed: true },
    { stdout: `${root}\n` },
    { stdout: `${root}\n` },
    { stdout: `${root}\n` },
  ]);
  assert.equal(await discoverStandalone(fake.exec, root, ".wtw"), undefined);
  assert.equal(await discoverStandalone(fake.exec, root, ".wtw"), undefined);
  mkdirSync(join(root, ".wtw"));
  writeFileSync(join(root, ".wtw", "SKILL.md"), "adopted");
  assert.equal(await discoverStandalone(fake.exec, root, ".wtw"), root);
  writeFileSync(join(root, "csm.toml"), "schema = 1\n");
  assert.equal(await discoverStandalone(fake.exec, root, ".wtw"), undefined);
  assert.ok(fake.calls.every((call) => call.command === "git"));
  rmSync(root, { recursive: true, force: true });
});

test("client uses exact argv/cwd and killed or truncated output never passes", async () => {
  const fake = harness([
    { stdout: "\u001b[31mrecord\u001b[0m\u001b" },
    { stdout: "partial", code: 0, killed: true },
    { stdout: "🚀".repeat(30_000) },
  ]);
  const client = new ToolClient({ repository: "/repo with space", binary: "custom-wtw", timeout: 321, exec: fake.exec });
  const first = await client.run(["explain", "--task", "literal ; value"]);
  assert.deepEqual(first, { ok: true, exitCode: 0, output: "record", truncated: false, killed: false });
  assert.deepEqual(fake.calls[0], { command: "custom-wtw", args: ["explain", "--task", "literal ; value"], options: { cwd: "/repo with space", timeout: 321, signal: undefined } });
  const killed = await client.run(["guard"]);
  assert.equal(killed.ok, false);
  assert.equal(killed.exitCode, 2);
  assert.equal(killed.killed, true);
  assert.match(killed.output, /terminated or timed out/);
  const large = await client.run(["explain"]);
  assert.equal(large.truncated, true);
  assert.ok(Buffer.byteLength(large.output) <= 64 * 1024);
  assert.doesNotMatch(large.output, /�/);
});
