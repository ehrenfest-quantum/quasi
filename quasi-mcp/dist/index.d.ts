#!/usr/bin/env node
/**
 * @quasi/mcp-server
 *
 * MCP server for the QUASI task board. Exposes the quasi-board ActivityPub
 * instance as Claude Code tools — list tasks, claim, complete, query ledger,
 * propose new tasks, and validate Ehrenfest programs.
 *
 * Default board: https://gawain.valiant-quantum.com
 * Override:      QUASI_BOARD_URL env var
 *
 * Usage in .mcp.json:
 *   { "mcpServers": { "quasi": { "command": "npx", "args": ["-y", "@quasi/mcp-server"] } } }
 */
export {};
