// One demo session: leases a command through the real allot-mcp, runs it, then releases it,
// as a Claude Code session does through its MCP tools. Each reply is printed as allotd sent it.
//   node session.mjs <allot-mcp> <cwd> <command> [args...]

import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import { createInterface } from "node:readline";

// allotd ends a lease whose holder is missing from its latest process sample, and a real
// session is always older than that; this one waits for a sample to include it.
const SAMPLED_AFTER_MS = 2500;

const [mcpPath, cwd, ...command] = process.argv.slice(2);
const label = command.join(" ");

// Claude Code gives each MCP server the session's ID; the parent process holds the lease.
const server = spawn(mcpPath, [], {
  cwd,
  env: { ...process.env, CLAUDE_CODE_SESSION_ID: randomUUID() },
  stdio: ["pipe", "pipe", "inherit"],
});

const pending = new Map();
let nextId = 1;

createInterface({ input: server.stdout }).on("line", (line) => {
  const message = JSON.parse(line);
  const resolve = pending.get(message.id);
  if (resolve) {
    pending.delete(message.id);
    resolve(message);
  }
});

function send(message) {
  server.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`);
}

function request(method, params) {
  const id = nextId++;
  return new Promise((resolve) => {
    pending.set(id, resolve);
    send({ id, method, params });
  });
}

async function callTool(name, args) {
  const reply = await request("tools/call", { name, arguments: args });
  const text = reply.result?.content?.[0]?.text;
  if (reply.error || reply.result?.isError || text === undefined) {
    throw new Error(`${name} failed: ${JSON.stringify(reply.error ?? reply.result)}`);
  }
  console.log(`${label} | ${name}: ${text}`);
  return JSON.parse(text);
}

function run(args) {
  return new Promise((resolve, reject) => {
    const child = spawn(args[0], args.slice(1), { cwd, stdio: "ignore" });
    child.on("error", reject);
    child.on("exit", (code) => (code === 0 ? resolve() : reject(new Error(`exit ${code}`))));
  });
}

await request("initialize", {
  protocolVersion: "2025-06-18",
  capabilities: {},
  clientInfo: { name: "allot-demo", version: "1" },
});
send({ method: "notifications/initialized" });
await new Promise((resolve) => setTimeout(resolve, SAMPLED_AFTER_MS));

let lease = await callTool("acquire_lease", { class: "cpu_heavy", label, cwd });
while (lease.state === "queued") {
  lease = await callTool("acquire_lease", { lease_id: lease.lease_id });
}
if (lease.state !== "active") {
  throw new Error(`lease ${lease.lease_id} ended as ${lease.state}`);
}
try {
  await run(command);
} finally {
  await callTool("release_lease", { lease_id: lease.lease_id });
  server.stdin.end();
}
