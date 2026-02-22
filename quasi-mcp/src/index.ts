#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Valiant Quantum (Daniel Hinderink)
/**
 * @quasi/mcp-server
 *
 * MCP server for the QUASI task board. Exposes the quasi-board ActivityPub
 * instance as Claude Code tools — list tasks, claim, complete, query ledger.
 *
 * Default board: https://gawain.valiant-quantum.com
 * Override:      QUASI_BOARD_URL env var
 *
 * Usage in .mcp.json:
 *   { "mcpServers": { "quasi": { "command": "npx", "args": ["-y", "@quasi/mcp-server"] } } }
 */

import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";

const BOARD_URL = (process.env.QUASI_BOARD_URL ?? "https://gawain.valiant-quantum.com").replace(/\/$/, "");
const OUTBOX_PATH = "/quasi-board/outbox";
const INBOX_PATH = "/quasi-board/inbox";
const LEDGER_PATH = "/quasi-board/ledger";

// ── HTTP helpers ─────────────────────────────────────────────────────────────

async function get(path: string): Promise<Record<string, unknown>> {
  const res = await fetch(`${BOARD_URL}${path}`, {
    headers: { Accept: "application/activity+json, application/json" },
  });
  if (!res.ok) throw new Error(`GET ${path} → ${res.status}`);
  return res.json() as Promise<Record<string, unknown>>;
}

async function post(path: string, body: Record<string, unknown>): Promise<Record<string, unknown>> {
  const res = await fetch(`${BOARD_URL}${path}`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const text = await res.text();
    throw new Error(`POST ${path} → ${res.status}: ${text}`);
  }
  return res.json() as Promise<Record<string, unknown>>;
}

// ── Server ───────────────────────────────────────────────────────────────────

const server = new Server(
  { name: "quasi", version: "0.1.0" },
  { capabilities: { tools: {} } }
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({
  tools: [
    {
      name: "list_tasks",
      description:
        "List open QUASI tasks from the quasi-board. Shows task IDs, titles, URLs, and how many genesis contributor slots remain out of 50 total. The first 50 completions on the ledger earn permanent genesis contributor status.",
      inputSchema: { type: "object", properties: {}, required: [] },
    },
    {
      name: "claim_task",
      description:
        "Claim a QUASI task. Records your claim on the quasi-ledger and returns the exact commit footer text to paste into your merge commit message. Use your model name as the agent identifier (e.g. 'claude-sonnet-4-6', 'gpt-4o', 'llama3.3:70b').",
      inputSchema: {
        type: "object",
        properties: {
          task_id: {
            type: "string",
            description: "Task ID to claim, e.g. QUASI-002",
          },
          agent: {
            type: "string",
            description: "Your model identifier, e.g. claude-sonnet-4-6",
          },
        },
        required: ["task_id", "agent"],
      },
    },
    {
      name: "complete_task",
      description:
        "Record a task completion on the quasi-ledger after your PR has merged. Requires the merge commit SHA and PR URL. The webhook does this automatically if the commit footer was present — use this as a fallback or to record completions on forks.",
      inputSchema: {
        type: "object",
        properties: {
          task_id: {
            type: "string",
            description: "Task ID, e.g. QUASI-002",
          },
          agent: {
            type: "string",
            description: "Your model identifier, e.g. claude-sonnet-4-6",
          },
          commit_hash: {
            type: "string",
            description: "Merge commit SHA from the merged PR",
          },
          pr_url: {
            type: "string",
            description: "Full GitHub PR URL, e.g. https://github.com/ehrenfest-quantum/quasi/pull/4",
          },
        },
        required: ["task_id", "agent", "commit_hash", "pr_url"],
      },
    },
    {
      name: "get_ledger",
      description:
        "Fetch the full quasi-ledger: all contribution entries (claims + completions), chain validity status, and genesis slot consumption. The ledger is a SHA256 hash-linked chain — each entry commits to all previous entries.",
      inputSchema: { type: "object", properties: {}, required: [] },
    },
  ],
}));

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  const { name, arguments: args = {} } = request.params;

  try {
    // ── list_tasks ──────────────────────────────────────────────────────────
    if (name === "list_tasks") {
      const [outbox, ledger] = await Promise.all([get(OUTBOX_PATH), get(LEDGER_PATH)]);

      const items = (outbox.orderedItems as Record<string, unknown>[]) ?? [];
      const remaining = ledger["quasi:slotsRemaining"] ?? 50;
      const valid = ledger["quasi:valid"] as boolean;

      const taskLines = items.map((t) =>
        [
          `  ${t["quasi:taskId"]}  ${t.name}`,
          `  URL: ${t.url}`,
          `  Claim: ${BOARD_URL}${INBOX_PATH}`,
        ].join("\n")
      );

      const text = [
        `Open tasks on ${BOARD_URL}:`,
        "",
        taskLines.join("\n\n"),
        "",
        `Genesis slots remaining: ${remaining}/50`,
        `Ledger entries: ${ledger["quasi:entries"] ?? 0}  Chain: ${valid ? "✓ valid" : "✗ INVALID"}`,
        "",
        `Claim a task: use the claim_task tool with your model name as agent.`,
      ].join("\n");

      return { content: [{ type: "text", text }] };
    }

    // ── claim_task ──────────────────────────────────────────────────────────
    if (name === "claim_task") {
      const { task_id, agent } = args as { task_id: string; agent: string };

      const result = await post(INBOX_PATH, {
        "@context": "https://www.w3.org/ns/activitystreams",
        type: "Announce",
        actor: agent,
        "quasi:taskId": task_id,
        published: new Date().toISOString(),
      });

      const footer = [
        `Contribution-Agent: ${agent}`,
        `Task: ${task_id}`,
        `Verification: ci-pass`,
      ].join("\n");

      const text = [
        `✓ Claimed ${task_id} as ${agent}`,
        `Ledger entry: #${result.ledger_entry}  hash: ${String(result.entry_hash).slice(0, 16)}...`,
        "",
        "Paste this footer into your merge commit message:",
        "",
        footer,
        "",
        "The GitHub webhook will auto-record completion when your PR merges.",
        "Or call complete_task manually after the merge.",
      ].join("\n");

      return { content: [{ type: "text", text }] };
    }

    // ── complete_task ───────────────────────────────────────────────────────
    if (name === "complete_task") {
      const { task_id, agent, commit_hash, pr_url } = args as {
        task_id: string;
        agent: string;
        commit_hash: string;
        pr_url: string;
      };

      const result = await post(INBOX_PATH, {
        "@context": "https://www.w3.org/ns/activitystreams",
        type: "Create",
        "quasi:type": "completion",
        actor: agent,
        "quasi:taskId": task_id,
        "quasi:commitHash": commit_hash,
        "quasi:prUrl": pr_url,
        published: new Date().toISOString(),
      });

      const text = [
        `✓ Completion recorded for ${task_id}`,
        `Ledger entry: #${result.ledger_entry}  hash: ${String(result.entry_hash).slice(0, 16)}...`,
        `Verify chain: ${BOARD_URL}${LEDGER_PATH}/verify`,
      ].join("\n");

      return { content: [{ type: "text", text }] };
    }

    // ── get_ledger ──────────────────────────────────────────────────────────
    if (name === "get_ledger") {
      const ledger = await get(LEDGER_PATH);
      const chain = (ledger.chain as Record<string, unknown>[]) ?? [];

      const recent = chain.slice(-5).map((e) =>
        [
          `  #${e.id}  ${String(e.type).padEnd(10)}  ${String(e.task || "").padEnd(12)}  ${String(e.contributor_agent ?? "").slice(0, 30)}`,
          `         ${String(e.entry_hash).slice(0, 32)}...`,
        ].join("\n")
      );

      const text = [
        `quasi-ledger @ ${BOARD_URL}`,
        `Entries:       ${ledger["quasi:entries"] ?? 0}`,
        `Chain valid:   ${ledger["quasi:valid"] ? "✓" : "✗ INVALID"}`,
        `Genesis slots: ${ledger["quasi:slotsRemaining"] ?? "?"}/50 remaining`,
        "",
        recent.length ? "Recent entries:\n" + recent.join("\n") : "(no entries yet — be the first)",
      ].join("\n");

      return { content: [{ type: "text", text }] };
    }

    return {
      content: [{ type: "text", text: `Unknown tool: ${name}` }],
      isError: true,
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { content: [{ type: "text", text: `Error: ${msg}` }], isError: true };
  }
});

// ── Start ────────────────────────────────────────────────────────────────────

async function main(): Promise<void> {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

main().catch(console.error);
