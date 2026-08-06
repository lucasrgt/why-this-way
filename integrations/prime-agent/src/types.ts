export interface ExecOptions {
  cwd?: string;
  timeout?: number;
  signal?: AbortSignal;
}

export interface ExecResult {
  stdout: string;
  stderr: string;
  code: number;
  killed: boolean;
}

export type Exec = (command: string, args: string[], options?: ExecOptions) => Promise<ExecResult>;

export interface ToolRun {
  ok: boolean;
  exitCode: number;
  output: string;
  truncated: boolean;
  killed: boolean;
}

export interface PrimeUi {
  notify(message: string, level?: "info" | "warning" | "error"): void;
  setStatus(key: string, value: string | undefined): void;
}

export interface PrimeContext {
  cwd: string;
  hasUI: boolean;
  signal?: AbortSignal;
  ui: PrimeUi;
}

export interface BeforeAgentStartEvent { prompt: string; }
export interface BeforeAgentStartResult {
  message?: { customType: string; content: string; display: boolean };
}
export interface ResourcesDiscoverResult { skillPaths?: string[]; }

export interface PrimeExtensionApi {
  registerFlag(name: string, options: { description?: string; type: "boolean" | "string"; default?: boolean | string }): void;
  getFlag(name: string): boolean | string | undefined;
  exec: Exec;
  sendMessage(message: { customType: string; content: string; display: boolean }, options?: { triggerTurn?: boolean }): void;
  registerCommand(name: string, options: {
    description?: string;
    getArgumentCompletions?(prefix: string): Array<{ value: string; label: string; description?: string }> | null;
    handler(args: string, context: PrimeContext): Promise<void> | void;
  }): void;
  on(event: "resources_discover", handler: (event: unknown, context: PrimeContext) => Promise<ResourcesDiscoverResult | void> | ResourcesDiscoverResult | void): void;
  on(event: "before_agent_start", handler: (event: BeforeAgentStartEvent, context: PrimeContext) => Promise<BeforeAgentStartResult | void> | BeforeAgentStartResult | void): void;
  on(event: string, handler: (event: unknown, context: PrimeContext) => Promise<unknown> | unknown): void;
}
