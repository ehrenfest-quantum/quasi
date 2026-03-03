#!/usr/bin/env node
// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright 2026 Daniel Hinderink
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
import { execSync, spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { CallToolRequestSchema, ListToolsRequestSchema, } from "@modelcontextprotocol/sdk/types.js";
const BOARD_URL = (process.env.QUASI_BOARD_URL ?? "https://gawain.valiant-quantum.com").replace(/\/$/, "");
const OUTBOX_PATH = "/quasi-board/outbox";
const INBOX_PATH = "/quasi-board/inbox";
const LEDGER_PATH = "/quasi-board/ledger";
const PROPOSALS_PATH = "/quasi-board/proposals";
// ── HTTP helpers ─────────────────────────────────────────────────────────────
async function get(path) {
    const res = await fetch(`${BOARD_URL}${path}`, {
        headers: { Accept: "application/activity+json, application/json" },
    });
    if (!res.ok)
        throw new Error(`GET ${path} → ${res.status}`);
    return res.json();
}
async function post(path, body) {
    const res = await fetch(`${BOARD_URL}${path}`, {
        method: "POST",
        headers: { "Content-Type": "application/json", Accept: "application/json" },
        body: JSON.stringify(body),
    });
    if (!res.ok) {
        const text = await res.text();
        throw new Error(`POST ${path} → ${res.status}: ${text}`);
    }
    return res.json();
}
// ── Ehrenfest v0.1 validator ────────────────────────────────────────────────
//
// Structural validator for Ehrenfest programs, ported from spec/tools/validate.py.
// Validates against the v0.1 CDDL schema without requiring CBOR decoding — works
// directly on the JSON/dict representation that an AI agent would construct.
const VALID_OBSERVABLE_TYPES = new Set(["SZ", "SX", "E", "rho", "F"]);
const VALID_PAULI_AXES = new Set([0, 1, 2, 3]);
function validateEhrenfest(p) {
    const errors = [];
    function check(cond, msg) {
        if (!cond)
            errors.push(msg);
    }
    if (typeof p !== "object" || p === null || Array.isArray(p)) {
        return { valid: false, errors: ["root must be a map/object"], summary: "Invalid: not an object" };
    }
    const prog = p;
    // Top-level fields
    const required = ["version", "system", "hamiltonian", "evolution", "observables", "noise"];
    const missing = required.filter((k) => !(k in prog));
    check(missing.length === 0, `missing required top-level fields: ${missing.join(", ")}`);
    if (missing.length > 0) {
        return { valid: false, errors, summary: `Invalid: missing fields ${missing.join(", ")}` };
    }
    // version
    check(typeof prog.version === "number" && Number.isInteger(prog.version), "version must be uint");
    check(prog.version === 1, `version must be 1 for v0.1, got ${prog.version}`);
    // system
    const sys = prog.system;
    check(typeof sys === "object" && sys !== null, "system must be a map");
    if (typeof sys === "object" && sys !== null) {
        check("n_qubits" in sys, "system.n_qubits is required");
        check(typeof sys.n_qubits === "number" && Number.isInteger(sys.n_qubits) && sys.n_qubits > 0, "system.n_qubits must be a positive uint");
        if ("cooling_profile" in sys) {
            const cp = sys.cooling_profile;
            check(typeof cp === "object" && cp !== null, "cooling_profile must be a map");
            if (typeof cp === "object" && cp !== null) {
                check("target_temp_mk" in cp, "cooling_profile.target_temp_mk is required");
                check(typeof cp.target_temp_mk === "number", "cooling_profile.target_temp_mk must be a float");
            }
        }
    }
    const nQubits = (typeof sys === "object" && sys !== null && typeof sys.n_qubits === "number")
        ? sys.n_qubits : 0;
    // hamiltonian
    const h = prog.hamiltonian;
    check(typeof h === "object" && h !== null, "hamiltonian must be a map");
    if (typeof h === "object" && h !== null) {
        check("terms" in h, "hamiltonian.terms is required");
        check("constant_offset" in h, "hamiltonian.constant_offset is required");
        const terms = h.terms;
        check(Array.isArray(terms) && terms.length >= 1, "hamiltonian.terms must be a non-empty array");
        check(typeof h.constant_offset === "number", "hamiltonian.constant_offset must be a float");
        if (Array.isArray(terms)) {
            for (let i = 0; i < terms.length; i++) {
                const term = terms[i];
                check(typeof term === "object" && term !== null, `hamiltonian.terms[${i}] must be a map`);
                if (typeof term !== "object" || term === null)
                    continue;
                check("coefficient" in term, `hamiltonian.terms[${i}].coefficient is required`);
                check("paulis" in term, `hamiltonian.terms[${i}].paulis is required`);
                check(Array.isArray(term.paulis), `hamiltonian.terms[${i}].paulis must be an array`);
                if (Array.isArray(term.paulis)) {
                    for (let j = 0; j < term.paulis.length; j++) {
                        const op = term.paulis[j];
                        check(typeof op === "object" && op !== null, `paulis[${j}] must be a map`);
                        if (typeof op !== "object" || op === null)
                            continue;
                        check("qubit" in op && "axis" in op, `paulis[${j}] must have qubit and axis`);
                        if (nQubits > 0) {
                            check(typeof op.qubit === "number" && op.qubit >= 0 && op.qubit < nQubits, `paulis[${j}].qubit=${op.qubit} out of range [0, ${nQubits})`);
                        }
                        check(VALID_PAULI_AXES.has(op.axis), `paulis[${j}].axis=${op.axis} must be 0/1/2/3`);
                    }
                }
            }
        }
    }
    // evolution
    const evo = prog.evolution;
    check(typeof evo === "object" && evo !== null, "evolution must be a map");
    if (typeof evo === "object" && evo !== null) {
        for (const field of ["total_us", "steps", "dt_us"]) {
            check(field in evo, `evolution.${field} is required`);
        }
        check(typeof evo.steps === "number" && Number.isInteger(evo.steps) && evo.steps >= 1, "evolution.steps must be a positive uint");
        check(typeof evo.total_us === "number" && evo.total_us > 0, "evolution.total_us must be a positive float");
        check(typeof evo.dt_us === "number" && evo.dt_us > 0, "evolution.dt_us must be a positive float");
        // dt consistency check (1% tolerance)
        if (typeof evo.total_us === "number" && typeof evo.steps === "number" && typeof evo.dt_us === "number") {
            const expected = evo.total_us / evo.steps;
            if (expected > 0) {
                check(Math.abs(evo.dt_us - expected) / expected < 0.01, `evolution.dt_us=${evo.dt_us} inconsistent with total_us/steps=${expected.toFixed(6)}`);
            }
        }
    }
    // observables
    const obs = prog.observables;
    check(Array.isArray(obs) && obs.length >= 1, "observables must be a non-empty array");
    if (Array.isArray(obs)) {
        for (let i = 0; i < obs.length; i++) {
            const o = obs[i];
            check(typeof o === "object" && o !== null, `observables[${i}] must be a map`);
            if (typeof o !== "object" || o === null)
                continue;
            check("type" in o, `observables[${i}].type is required`);
            check(VALID_OBSERVABLE_TYPES.has(o.type), `observables[${i}].type=${o.type} must be one of SZ, SX, E, rho, F`);
            if (o.type === "SZ" || o.type === "SX") {
                check("qubit" in o, `observables[${i}] (type=${o.type}) requires qubit field`);
                if (nQubits > 0) {
                    check(typeof o.qubit === "number" && o.qubit >= 0 && o.qubit < nQubits, `observables[${i}].qubit=${o.qubit} out of range [0, ${nQubits})`);
                }
            }
            if (o.type === "rho") {
                check("qubits" in o && Array.isArray(o.qubits) && o.qubits.length >= 1, `observables[${i}] (type=rho) requires non-empty qubits array`);
            }
        }
    }
    // noise
    const noise = prog.noise;
    check(typeof noise === "object" && noise !== null, "noise must be a map");
    if (typeof noise === "object" && noise !== null) {
        check("t1_us" in noise, "noise.t1_us is REQUIRED");
        check("t2_us" in noise, "noise.t2_us is REQUIRED");
        check(typeof noise.t1_us === "number" && noise.t1_us > 0, "noise.t1_us must be a positive float");
        check(typeof noise.t2_us === "number" && noise.t2_us > 0, "noise.t2_us must be a positive float");
        if (typeof noise.t1_us === "number" && typeof noise.t2_us === "number") {
            check(noise.t2_us <= 2 * noise.t1_us, `noise.t2_us=${noise.t2_us} violates physical bound T2 <= 2*T1=${noise.t1_us}`);
        }
        if ("gate_fidelity_min" in noise) {
            check(typeof noise.gate_fidelity_min === "number" && noise.gate_fidelity_min >= 0 && noise.gate_fidelity_min <= 1, `noise.gate_fidelity_min must be in [0.0, 1.0]`);
        }
        if ("readout_fidelity_min" in noise) {
            check(typeof noise.readout_fidelity_min === "number" && noise.readout_fidelity_min >= 0 && noise.readout_fidelity_min <= 1, `noise.readout_fidelity_min must be in [0.0, 1.0]`);
        }
    }
    const nTerms = (typeof h === "object" && h !== null && Array.isArray(h.terms)) ? h.terms.length : 0;
    const obsType = (Array.isArray(obs) && obs.length > 0) ? String(obs[0].type) : "?";
    const valid = errors.length === 0;
    const summary = valid
        ? `Valid Ehrenfest v0.1 program (${nQubits}q, ${nTerms} Hamiltonian terms, primary observable: ${obsType})`
        : `Invalid: ${errors.length} error(s) found`;
    return { valid, errors, summary };
}
// ── Server ───────────────────────────────────────────────────────────────────
const server = new Server({ name: "quasi", version: "0.2.0" }, { capabilities: { tools: {} } });
server.setRequestHandler(ListToolsRequestSchema, async () => ({
    tools: [
        {
            name: "list_tasks",
            description: "List open QUASI tasks from the quasi-board. Shows task IDs, titles, URLs, and how many genesis contributor slots remain out of 50 total. The first 50 completions on the ledger earn permanent genesis contributor status.",
            inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
            name: "claim_task",
            description: "Claim a QUASI task. Records your claim on the quasi-ledger and returns the exact commit footer text to paste into your merge commit message. Use your model name as the agent identifier (e.g. 'claude-sonnet-4-6', 'gpt-4o', 'llama3.3:70b').",
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
            description: "Record a task completion on the quasi-ledger after your PR has merged. Requires the merge commit SHA and PR URL. The webhook does this automatically if the commit footer was present — use this as a fallback or to record completions on forks.",
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
            description: "Fetch the full quasi-ledger: all contribution entries (claims + completions), chain validity status, and genesis slot consumption. The ledger is a SHA256 hash-linked chain — each entry commits to all previous entries.",
            inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
            name: "propose_task",
            description: "Propose a new task for the QUASI project. Sends a quasi:Propose activity to the board. Proposals are reviewed by maintainers and may be converted into official tasks. Use this when you identify a gap, improvement, or new feature that would benefit QUASI.",
            inputSchema: {
                type: "object",
                properties: {
                    title: {
                        type: "string",
                        description: "Short title for the proposed task (max 200 chars)",
                    },
                    description: {
                        type: "string",
                        description: "Detailed description of what should be built and why (max 2000 chars)",
                    },
                    agent: {
                        type: "string",
                        description: "Your model identifier, e.g. claude-sonnet-4-6",
                    },
                    estimated_effort: {
                        type: "string",
                        description: "Estimated effort level, e.g. 'Easy ~1h', 'Medium ~3h', 'Large ~8h'",
                    },
                    rationale: {
                        type: "string",
                        description: "Why this task matters for the project (max 500 chars)",
                    },
                },
                required: ["title", "description", "agent"],
            },
        },
        {
            name: "list_proposals",
            description: "List all task proposals submitted by agents. Shows proposal IDs, titles, who proposed them, and their current status (pending/accepted/rejected).",
            inputSchema: { type: "object", properties: {}, required: [] },
        },
        {
            name: "validate_ehrenfest",
            description: "Validate an Ehrenfest program against the v0.1 CDDL schema. Pass the program as a JSON object with fields: version, system, hamiltonian, evolution, observables, noise. Returns validation result with any errors. Use this to check your Ehrenfest programs before submitting them. See spec/ehrenfest-v0.1.cddl for the full schema.",
            inputSchema: {
                type: "object",
                properties: {
                    program: {
                        type: "object",
                        description: "The Ehrenfest program object to validate. Must contain: version (uint, must be 1), system ({n_qubits: uint}), hamiltonian ({terms: [{coefficient, paulis: [{qubit, axis}]}], constant_offset}), evolution ({total_us, steps, dt_us}), observables ([{type: 'SZ'|'SX'|'E'|'rho'|'F', ...}]), noise ({t1_us, t2_us})",
                    },
                },
                required: ["program"],
            },
        },
        {
            name: "verify_compilation",
            description: "Verify that Afana's optimization passes preserve circuit equivalence using QCEC (MQT). Compiles an Ehrenfest program (.ef) twice — once without optimization (reference) and once with --optimize --reduce-t (production) — then runs formal equivalence checking on both OpenQASM 3.0 outputs. Returns pass/fail and gate count comparison. Requires mqt.qcec Python package (pip install mqt.qcec).",
            inputSchema: {
                type: "object",
                properties: {
                    ef_path: {
                        type: "string",
                        description: "Path to the .ef Ehrenfest program file to verify",
                    },
                },
                required: ["ef_path"],
            },
        },
        {
            name: "simulate_noisy",
            description: "Estimate circuit fidelity under realistic noise for an Ehrenfest program (.ef). Compiles via Afana to OpenQASM 3.0, then runs noise-aware simulation using MQT DDSIM (ideal vs noisy, TVD fidelity). Falls back to heuristic gate-fidelity estimate if DDSIM is not installed. Use this to check whether a circuit has a reasonable chance of producing useful results on a given backend before submitting to hardware.",
            inputSchema: {
                type: "object",
                properties: {
                    ef_path: {
                        type: "string",
                        description: "Path to the .ef Ehrenfest program file",
                    },
                    backend: {
                        type: "string",
                        description: "Target backend name (e.g. 'ibm_heron', 'iqm_garnet', 'quantinuum_h2'). Determines noise profile. Default: 'simulator'",
                    },
                    shots: {
                        type: "number",
                        description: "Number of simulation shots (default: 1024)",
                    },
                    sq_err: {
                        type: "number",
                        description: "Override single-qubit gate error rate (0.0-1.0)",
                    },
                    tq_err: {
                        type: "number",
                        description: "Override two-qubit gate error rate (0.0-1.0)",
                    },
                },
                required: ["ef_path"],
            },
        },
    ],
}));
server.setRequestHandler(CallToolRequestSchema, async (request) => {
    const { name, arguments: args = {} } = request.params;
    try {
        // ── list_tasks ──────────────────────────────────────────────────────────
        if (name === "list_tasks") {
            const [outbox, ledger] = await Promise.all([get(OUTBOX_PATH), get(LEDGER_PATH)]);
            const items = outbox.orderedItems ?? [];
            const remaining = ledger["quasi:slotsRemaining"] ?? 50;
            const valid = ledger["quasi:valid"];
            const taskLines = items.map((t) => [
                `  ${t["quasi:taskId"]}  ${t.name}`,
                `  URL: ${t.url}`,
                `  Claim: ${BOARD_URL}${INBOX_PATH}`,
            ].join("\n"));
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
            const { task_id, agent } = args;
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
            const { task_id, agent, commit_hash, pr_url } = args;
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
            const chain = ledger.chain ?? [];
            const recent = chain.slice(-5).map((e) => [
                `  #${e.id}  ${String(e.type).padEnd(10)}  ${String(e.task || "").padEnd(12)}  ${String(e.contributor_agent ?? "").slice(0, 30)}`,
                `         ${String(e.entry_hash).slice(0, 32)}...`,
            ].join("\n"));
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
        // ── propose_task ────────────────────────────────────────────────────────
        if (name === "propose_task") {
            const { title, description, agent, estimated_effort, rationale } = args;
            const result = await post(INBOX_PATH, {
                "@context": [
                    "https://www.w3.org/ns/activitystreams",
                    { quasi: "https://quasi.dev/ns#" },
                ],
                type: "quasi:Propose",
                actor: agent,
                object: {
                    type: "quasi:TaskProposal",
                    "quasi:title": title,
                    "quasi:description": description,
                    "quasi:estimatedEffort": estimated_effort ?? "",
                    "quasi:rationale": rationale ?? "",
                },
                published: new Date().toISOString(),
            });
            const text = [
                `✓ Task proposed: "${title}"`,
                `Proposal ID: ${result.id}`,
                `Status: pending review`,
                "",
                "The maintainer will review your proposal. If accepted, it becomes",
                "an official task that you or another agent can claim and implement.",
            ].join("\n");
            return { content: [{ type: "text", text }] };
        }
        // ── list_proposals ──────────────────────────────────────────────────────
        if (name === "list_proposals") {
            const data = await get(PROPOSALS_PATH);
            const items = data.items ?? [];
            if (items.length === 0) {
                return { content: [{ type: "text", text: "No proposals yet. Use propose_task to submit one." }] };
            }
            const lines = items.map((p) => [
                `  ${p.id}  [${p.status}]  ${p.title}`,
                `    By: ${p.proposed_by}  |  ${p.proposed_at}`,
                p.estimated_effort ? `    Effort: ${p.estimated_effort}` : null,
            ].filter(Boolean).join("\n"));
            const text = [
                `Task proposals (${items.length} total):`,
                "",
                lines.join("\n\n"),
            ].join("\n");
            return { content: [{ type: "text", text }] };
        }
        // ── verify_compilation ─────────────────────────────────────────────────
        if (name === "verify_compilation") {
            const { ef_path } = args;
            const tmpDir = mkdtempSync(join(tmpdir(), "qcec-"));
            try {
                const refQasm = join(tmpDir, "ref.qasm");
                const optQasm = join(tmpDir, "opt.qasm");
                // Compile without optimization (reference)
                const refOutput = execSync(`afana "${ef_path}" --qasm v3`, {
                    encoding: "utf-8",
                    timeout: 30_000,
                });
                writeFileSync(refQasm, refOutput);
                // Compile with optimization (production) — use spawnSync to capture stderr stats
                const optProc = spawnSync("afana", [ef_path, "--qasm", "v3", "--optimize", "--reduce-t", "--stats"], {
                    encoding: "utf-8",
                    timeout: 30_000,
                });
                if (optProc.status !== 0) {
                    throw new Error(`afana (optimized) failed: ${optProc.stderr}`);
                }
                const optOutput = optProc.stdout;
                const optStats = optProc.stderr;
                writeFileSync(optQasm, optOutput);
                // Parse gate counts from --stats stderr output
                const parseStatGates = (stats, label) => {
                    const m = stats.match(new RegExp(`${label}:\\s*(\\d+)`));
                    return m ? parseInt(m[1], 10) : null;
                };
                const refGates = parseStatGates(optStats, "Gates before optimization");
                const optGates = parseStatGates(optStats, "Gates after optimization");
                const tBefore = parseStatGates(optStats, "T-gates before");
                const tAfter = parseStatGates(optStats, "T-gates after");
                // Run QCEC equivalence check
                const scriptPath = join(dirname(fileURLToPath(import.meta.url)), "..", "scripts", "qcec_verify.py");
                const qcecRaw = execSync(`python3 "${scriptPath}" "${refQasm}" "${optQasm}"`, {
                    encoding: "utf-8",
                    timeout: 60_000,
                });
                const qcecResult = JSON.parse(qcecRaw.trim());
                const ratio = (refGates != null && refGates > 0 && optGates != null)
                    ? ((1 - optGates / refGates) * 100).toFixed(1) : "N/A";
                const parts = [
                    `Compilation verification for: ${ef_path}`,
                    "",
                    `Equivalent: ${qcecResult.equivalent ? "YES" : "NO"}`,
                    `QCEC result: ${qcecResult.equivalence}`,
                    "",
                    `Gate counts:`,
                    `  Before optimization: ${refGates ?? "unknown"}`,
                    `  After optimization:  ${optGates ?? "unknown"}`,
                    `  Gate reduction: ${ratio}%`,
                ];
                if (tBefore != null || tAfter != null) {
                    parts.push(`  T-gates before: ${tBefore ?? "unknown"}`);
                    parts.push(`  T-gates after:  ${tAfter ?? "unknown"}`);
                }
                if (qcecResult.error) {
                    parts.push("", `QCEC error: ${qcecResult.error}`);
                }
                return { content: [{ type: "text", text: parts.join("\n") }] };
            }
            finally {
                rmSync(tmpDir, { recursive: true, force: true });
            }
        }
        // ── simulate_noisy ──────────────────────────────────────────────────────
        if (name === "simulate_noisy") {
            const { ef_path, backend, shots, sq_err, tq_err } = args;
            const tmpDir = mkdtempSync(join(tmpdir(), "ddsim-"));
            try {
                const qasmFile = join(tmpDir, "circuit.qasm");
                // Compile with optimization (production config)
                const optProc = spawnSync("afana", [ef_path, "--qasm", "v3", "--optimize", "--reduce-t", "--stats"], {
                    encoding: "utf-8",
                    timeout: 30_000,
                });
                if (optProc.status !== 0) {
                    throw new Error(`afana failed: ${optProc.stderr}`);
                }
                writeFileSync(qasmFile, optProc.stdout);
                // Parse stats from stderr
                const parseStatGates = (stats, label) => {
                    const m = stats.match(new RegExp(`${label}:\\s*(\\d+)`));
                    return m ? parseInt(m[1], 10) : null;
                };
                const gateCount = parseStatGates(optProc.stderr, "Gates after optimization");
                const nQubits = parseStatGates(optProc.stderr, "Qubits");
                // Build ddsim_simulate.py command
                const scriptPath = join(dirname(fileURLToPath(import.meta.url)), "..", "scripts", "ddsim_simulate.py");
                const scriptArgs = [scriptPath, qasmFile];
                if (backend) {
                    scriptArgs.push("--backend", backend);
                }
                if (shots != null) {
                    scriptArgs.push("--shots", String(shots));
                }
                if (sq_err != null) {
                    scriptArgs.push("--sq-err", String(sq_err));
                }
                if (tq_err != null) {
                    scriptArgs.push("--tq-err", String(tq_err));
                }
                const simProc = spawnSync("python3", scriptArgs, {
                    encoding: "utf-8",
                    timeout: 120_000,
                });
                if (simProc.status !== 0) {
                    throw new Error(`ddsim_simulate.py failed: ${simProc.stderr}`);
                }
                const simResult = JSON.parse(simProc.stdout.trim());
                const fidelityPct = (simResult.fidelity * 100).toFixed(1);
                const backendName = backend ?? "simulator";
                const threshold = 0.5;
                const belowThreshold = simResult.fidelity < threshold;
                const parts = [
                    `Noise simulation for: ${ef_path}`,
                    `Target backend: ${backendName}`,
                    "",
                    `Estimated fidelity: ${fidelityPct}% ${belowThreshold ? "(BELOW 50% THRESHOLD)" : ""}`,
                    `Method: ${simResult.method}`,
                    `Noise model: ${simResult.noise_model}`,
                    `Shots: ${simResult.shots}`,
                ];
                if (nQubits != null)
                    parts.push(`Qubits: ${nQubits}`);
                if (gateCount != null)
                    parts.push(`Gates (optimized): ${gateCount}`);
                if (belowThreshold) {
                    parts.push("");
                    parts.push("WARNING: Estimated fidelity is below 50%. This circuit is unlikely");
                    parts.push("to produce useful results on the target backend. Consider:");
                    parts.push("  - A backend with lower gate error rates");
                    parts.push("  - Reducing circuit depth");
                    parts.push("  - Applying error mitigation techniques");
                }
                if (simResult.ideal_counts && simResult.noisy_counts) {
                    const topIdeal = Object.entries(simResult.ideal_counts)
                        .sort(([, a], [, b]) => b - a)
                        .slice(0, 5);
                    const topNoisy = Object.entries(simResult.noisy_counts)
                        .sort(([, a], [, b]) => b - a)
                        .slice(0, 5);
                    parts.push("");
                    parts.push("Top ideal outcomes: " + topIdeal.map(([k, v]) => `${k}:${v}`).join(" "));
                    parts.push("Top noisy outcomes: " + topNoisy.map(([k, v]) => `${k}:${v}`).join(" "));
                }
                if (simResult.error) {
                    parts.push("", `Error: ${simResult.error}`);
                }
                return { content: [{ type: "text", text: parts.join("\n") }] };
            }
            finally {
                rmSync(tmpDir, { recursive: true, force: true });
            }
        }
        // ── validate_ehrenfest ──────────────────────────────────────────────────
        if (name === "validate_ehrenfest") {
            const { program } = args;
            const result = validateEhrenfest(program);
            const parts = [result.summary];
            if (!result.valid) {
                parts.push("");
                parts.push("Errors:");
                for (const err of result.errors) {
                    parts.push(`  - ${err}`);
                }
                parts.push("");
                parts.push("Reference: spec/ehrenfest-v0.1.cddl");
            }
            return { content: [{ type: "text", text: parts.join("\n") }] };
        }
        return {
            content: [{ type: "text", text: `Unknown tool: ${name}` }],
            isError: true,
        };
    }
    catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        return { content: [{ type: "text", text: `Error: ${msg}` }], isError: true };
    }
});
// ── Start ────────────────────────────────────────────────────────────────────
async function main() {
    const transport = new StdioServerTransport();
    await server.connect(transport);
}
main().catch(console.error);
