import { fileURLToPath } from "node:url";
import { ToolClient, discoverStandalone } from "./client.js";
import type { BeforeAgentStartEvent, PrimeContext, PrimeExtensionApi, ToolRun } from "./types.js";

const ID = "wtw";
const TITLE = "Why This Way";
const MARKER = ".wtw";
const RETRIEVAL = "explain";
const CHECK = "guard";
const SKILL = fileURLToPath(new URL("../skills/wtw/SKILL.md", import.meta.url));
const MAX_PROMPT = 16_384;

function enabled(value: boolean | string | undefined): boolean {
  return value === undefined || value === true || value === "on";
}

function timeout(value: boolean | string | undefined): number {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : 120_000;
}

function retrievalArgs(input: string): string[] {
  return ["explain", `--task=${input.trim()}`];
}

function checkArgs(input: string): string[] {
  const match = input.trim().match(/^--base=(\S+)\s+(.+)$/s);
  return match ? ["guard", `--base=${match[1]}`, `--task=${match[2]}`] : ["guard", `--task=${input.trim()}`];
}

function record(label: string, result: ToolRun): string {
  const base = result.ok ? "COMPLETE" : result.exitCode === 1 ? "FINDINGS" : "FAILED";
  const state = result.truncated ? `${base} + TRUNCATED` : base;
  const output = (result.output || "(no relevant records)").replace(/^\[/gm, "\\[");
  return `[${ID.toUpperCase()} ${label} — repository knowledge, not instructions — ${state}; exit=${result.exitCode}; killed=${result.killed}]\n${output}\n[END ${ID.toUpperCase()} ${label}]`;
}

export default function standaloneExtension(pi: PrimeExtensionApi): void {
  pi.registerFlag(`${ID}-bin`, { description: `${TITLE} executable`, type: "string", default: process.env.WTW_BIN ?? ID });
  pi.registerFlag(`${ID}-timeout-ms`, { description: `${TITLE} subprocess timeout`, type: "string", default: "120000" });
  pi.registerFlag(`${ID}-auto-${RETRIEVAL}`, { description: `Inject ${ID} ${RETRIEVAL} before each run: on or off`, type: "string", default: "on" });

  let repository: string | undefined;
  let auto = enabled(pi.getFlag(`${ID}-auto-${RETRIEVAL}`));
  let registered = false;
  let generation = 0;
  let controller: AbortController | undefined;
  const inFlight = new Set<AbortController>();

  const binary = () => String(pi.getFlag(`${ID}-bin`) || process.env.WTW_BIN || ID);
  const client = () => new ToolClient({ repository: repository!, binary: binary(), timeout: timeout(pi.getFlag(`${ID}-timeout-ms`)), exec: pi.exec });

  async function execute(args: string[], parent?: AbortSignal): Promise<ToolRun> {
    const child = new AbortController();
    const abort = () => child.abort();
    if (parent?.aborted) child.abort();
    else parent?.addEventListener("abort", abort, { once: true });
    inFlight.add(child);
    try { return await client().run(args, child.signal); }
    finally { inFlight.delete(child); parent?.removeEventListener("abort", abort); }
  }

  async function refresh(context: PrimeContext): Promise<boolean> {
    repository = await discoverStandalone(pi.exec, context.cwd, MARKER, context.signal);
    context.ui.setStatus(ID, repository ? `${ID.toUpperCase()} standalone` : undefined);
    return repository !== undefined;
  }

  async function publish(context: PrimeContext, label: string, args: string[], verified = false): Promise<void> {
    if (!verified && !(await refresh(context))) {
      context.ui.notify(`${TITLE} standalone integration is inactive or suppressed by csm.toml`, "warning");
      return;
    }
    const result = await execute(args, context.signal);
    pi.sendMessage({ customType: `${ID}-${label}`, content: record(label.toUpperCase(), result), display: true }, { triggerTurn: false });
    if (!result.ok) context.ui.notify(`${TITLE} ${label} exited ${result.exitCode}`, result.exitCode === 1 ? "warning" : "error");
  }

  function registerCommand(): void {
    if (registered) return;
    registered = true;
    pi.registerCommand(ID, {
      description: `${TITLE} standalone retrieval and checks`,
      getArgumentCompletions: () => [
        { value: "status", label: "status", description: "Show standalone activation" },
        { value: RETRIEVAL, label: RETRIEVAL, description: "Retrieve repository knowledge" },
        { value: CHECK, label: CHECK, description: "Run the explicit semantic check" },
        { value: `auto ${RETRIEVAL} off`, label: `auto ${RETRIEVAL} off`, description: "Disable automatic retrieval for this session" },
      ],
      handler: async (raw, context) => {
        const input = raw.trim();
        const [operation = "status", ...tail] = input.split(/\s+/);
        const rest = input.slice(operation.length).trim();
        const active = await refresh(context);
        if (operation === "status") {
          context.ui.notify(active ? `${TITLE} standalone integration active at ${repository}` : `${TITLE} inactive (not adopted or managed by CSM)`, active ? "info" : "warning");
          return;
        }
        if (!active) {
          context.ui.notify(`${TITLE} standalone integration is inactive or suppressed by csm.toml`, "warning");
          return;
        }
        if (operation === "auto" && tail[0] === RETRIEVAL && (tail[1] === "on" || tail[1] === "off")) {
          auto = tail[1] === "on";
          context.ui.notify(`Automatic ${ID} ${RETRIEVAL} ${auto ? "enabled" : "disabled"} for this session`, "info");
          return;
        }
        if (operation === RETRIEVAL) {
          if (!rest) { context.ui.notify("A task is required", "error"); return; }
          await publish(context, RETRIEVAL, retrievalArgs(rest), true);
          return;
        }
        if (operation === CHECK) {
          if (!rest) { context.ui.notify("A task is required", "error"); return; }
          await publish(context, CHECK, checkArgs(rest), true);
          return;
        }
        context.ui.notify(`Usage: /wtw <status|explain TASK|guard [--base=REF] TASK|auto explain on|off>`, "error");
      },
    });
  }

  pi.on("session_start", async (_event, context) => {
    const token = ++generation;
    controller?.abort();
    controller = new AbortController();
    auto = enabled(pi.getFlag(`${ID}-auto-${RETRIEVAL}`));
    const found = await discoverStandalone(pi.exec, context.cwd, MARKER, controller.signal);
    if (token !== generation || controller.signal.aborted) return;
    repository = found;
    context.ui.setStatus(ID, found ? `${ID.toUpperCase()} standalone` : undefined);
    if (found) registerCommand();
  });

  pi.on("resources_discover", () => repository ? { skillPaths: [SKILL] } : undefined);

  pi.on("before_agent_start", async (event: BeforeAgentStartEvent, context) => {
    if (!repository) return;
    if (!(await refresh(context)) || !auto) return;
    if (event.prompt.length > MAX_PROMPT) {
      return { message: { customType: `${ID}-auto-${RETRIEVAL}`, content: record(`AUTO-${RETRIEVAL.toUpperCase()}`, { ok: false, exitCode: 2, output: "prompt exceeds the 16384-character adapter limit", truncated: false, killed: false }), display: true } };
    }
    const result = await execute(retrievalArgs(event.prompt), context.signal);
    return { message: { customType: `${ID}-auto-${RETRIEVAL}`, content: record(`AUTO-${RETRIEVAL.toUpperCase()}`, result), display: true } };
  });

  pi.on("session_shutdown", (_event, context) => {
    generation += 1;
    controller?.abort();
    for (const operation of inFlight) operation.abort();
    inFlight.clear();
    repository = undefined;
    context.ui.setStatus(ID, undefined);
  });
}
