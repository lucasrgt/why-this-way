import { lstatSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import type { Exec, ExecResult, ToolRun } from "./types.js";

const MAX_OUTPUT = 64 * 1024;
const ANSI = /[\u001b\u009b](?:\][^\u0007]*(?:\u0007|\u001b\\)|[[\]()#;?]*(?:(?:(?:[a-zA-Z\d]*(?:;[-a-zA-Z\d\/#&.:=?%@~_]+)*)?\u0007)|(?:(?:\d{1,4}(?:[;:]\d{0,4})*)?[\dA-PR-TZcf-nq-uy=><~])))/g;
const CONTROLS = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/g;

function bounded(value: string): { output: string; truncated: boolean } {
  const sanitized = value.replace(ANSI, "").replace(CONTROLS, "").trim();
  const encoded = Buffer.from(sanitized);
  if (encoded.byteLength <= MAX_OUTPUT) return { output: sanitized, truncated: false };
  const suffix = "\n[output truncated]";
  let prefix = encoded.subarray(0, MAX_OUTPUT - Buffer.byteLength(suffix)).toString("utf8");
  if (prefix.endsWith("�")) prefix = prefix.slice(0, -1);
  return { output: `${prefix}${suffix}`, truncated: true };
}

function entryExists(path: string): boolean {
  try { lstatSync(path); return true; } catch { return false; }
}

function regularFile(path: string): boolean {
  try { return lstatSync(path).isFile(); } catch { return false; }
}

export async function discoverStandalone(exec: Exec, cwd: string, marker: string, signal?: AbortSignal): Promise<string | undefined> {
  let result: ExecResult;
  try {
    result = await exec("git", ["-C", cwd, "rev-parse", "--show-toplevel"], { cwd, timeout: 10_000, signal });
  } catch {
    return undefined;
  }
  const value = result.stdout.trim();
  if (result.killed || result.code !== 0 || !value || !isAbsolute(value)) return undefined;
  const root = resolve(value);
  if (entryExists(join(root, "csm.toml"))) return undefined;
  return regularFile(join(root, marker, "SKILL.md")) ? root : undefined;
}

export class ToolClient {
  constructor(private readonly options: { repository: string; binary: string; timeout: number; exec: Exec }) {}

  async run(args: string[], signal?: AbortSignal): Promise<ToolRun> {
    let result: ExecResult;
    try {
      result = await this.options.exec(this.options.binary, args, {
        cwd: this.options.repository,
        timeout: this.options.timeout,
        signal,
      });
    } catch (error) {
      result = { stdout: "", stderr: error instanceof Error ? error.message : String(error), code: 2, killed: false };
    }
    const terminated = result.killed ? "process was terminated or timed out" : "";
    const cleaned = bounded([result.stdout, result.stderr, terminated].filter(Boolean).join("\n"));
    return {
      ok: result.code === 0 && !result.killed,
      exitCode: result.killed && result.code === 0 ? 2 : result.code,
      output: cleaned.output,
      truncated: cleaned.truncated,
      killed: result.killed,
    };
  }
}
