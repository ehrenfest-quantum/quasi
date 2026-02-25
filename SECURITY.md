# Security Requirements for QUASI

All contributions to QUASI — features, fixes, docs — must satisfy baseline security standards. These are non-negotiable. Issues and PRs that violate these will be blocked until remediation.

---

## Baseline Security Principles (All Issues)

Every feature, CLI command, API endpoint, or configuration change must address:

### 1. Input Validation — No Arbitrary Input

**Principle:** Never trust user input. Validate everything.

**Applies to:**
- CLI arguments (flags, positional args)
- HTTP request bodies
- Configuration file values
- Environment variables used in logic
- File paths, URLs, command strings

**Checklist:**
- [ ] Input format is explicitly defined (e.g., "UUID v4 only", "HTTPS URLs only")
- [ ] Input is validated against schema on receipt
- [ ] Invalid input is rejected with clear error (no fallback parsing)
- [ ] No string interpolation into system calls or SQL queries
- [ ] File paths are resolved relative to a safe root (no `../../` escape)

**Examples — ✅ Safe:**
```python
import uuid
task_id = args.task_id
if not uuid.UUID(task_id, version=4):  # Strict format validation
    raise ValueError(f"Invalid task ID: {task_id}")
```

**Examples — ❌ Unsafe:**
```python
board_url = args.board  # No validation — SSRF risk
board_url = f"http://{args.domain}"  # Assumes user provides valid domain
```

---

### 2. Network Security — HTTPS Only

**Principle:** All network communication must be encrypted and authenticated.

**Checklist:**
- [ ] All URLs use `https://` (never `http://`)
- [ ] TLS certificate validation enabled (default for Python `requests` library)
- [ ] No self-signed certificates accepted in production
- [ ] Pinning recommended for critical endpoints (board, ledger)
- [ ] Timeout set on all HTTP calls (no hang attacks)
- [ ] No credentials in URLs (`https://user:pass@...` forbidden)

**Examples — ✅ Safe:**
```python
import requests
requests.get("https://quasi-board.example.com/tasks", 
    timeout=5, 
    verify=True)  # TLS verification on
```

**Examples — ❌ Unsafe:**
```python
requests.get(f"http://{board_url}/tasks")  # HTTP + arbitrary URL
requests.get(board_url, verify=False)  # Cert validation disabled
```

---

### 3. Configuration Security — No Runtime Injection

**Principle:** Configuration is immutable. No runtime URL/credential injection.

**Checklist:**
- [ ] Configuration loaded from file only (not CLI, env vars, or HTTP)
- [ ] Config file permissions are strict (mode 0600 on \*nix)
- [ ] No embedding credentials in URLs
- [ ] Secrets stored in environment (QUASI_BOARD_TOKEN, etc.), never in code
- [ ] Config schema is explicit (JSON Schema, Pydantic, TOML with typing)
- [ ] Config changes require restart (no hot-reload of sensitive settings)

**Examples — ✅ Safe:**
```python
# ~/.quasi/config.toml (mode 0600)
[board]
url = "https://quasi-board.valiant-quantum.com"

# Code reads it once at startup
import tomllib
with open(os.path.expanduser("~/.quasi/config.toml")) as f:
    config = tomllib.load(f)
BOARD_URL = config["board"]["url"]
```

**Examples — ❌ Unsafe:**
```python
# CLI injection
board_url = sys.argv[1]  # Arbitrary URL from command line

# Env var injection
board_url = os.getenv("QUASI_BOARD_URL", "default")  # Default means fallback to user input
```

---

### 4. No Hardcoded Secrets

**Principle:** Credentials, keys, and tokens are never in source code.

**Checklist:**
- [ ] No API keys, tokens, or passwords in `.py`, `.rs`, `.md`, or config files
- [ ] `.gitignore` excludes `*.env`, `**/config/`, `**/.secrets/`
- [ ] Credentials loaded from environment or secure storage only
- [ ] Each environment (dev, staging, prod) has separate credentials
- [ ] Credential rotation policy documented (if applicable)

**Safe patterns:**
```python
QUASI_BOARD_TOKEN = os.getenv("QUASI_BOARD_TOKEN")
if not QUASI_BOARD_TOKEN:
    raise RuntimeError("QUASI_BOARD_TOKEN not set. See SECURITY.md.")
```

---

### 5. Audit Logging — Privileged Operations

**Principle:** Sensitive actions are logged and traceable.

**Applies to:**
- Task claims / completions
- Configuration changes (restart, URL changes)
- Authentication failures
- Unusual network activity (retries, timeouts, 5xx errors)

**Checklist:**
- [ ] Log includes: timestamp, user/agent ID, action, resource, result (success/fail)
- [ ] Logs are not accessible to untrusted users
- [ ] Sensitive values (tokens, passwords) are NOT logged
- [ ] Log format is machine-parseable (JSON preferred)

**Example:**
```python
import logging
import json

logger = logging.getLogger("quasi_agent")
logger.info(json.dumps({
    "timestamp": datetime.utcnow().isoformat(),
    "action": "claim_task",
    "task_id": task_id,
    "agent": agent_id,
    "board": board_hostname,  # Not full URL
    "result": "success"
}))
```

---

### 6. Cryptographic Integrity — Board & Ledger Trust

**Principle:** Task ledger entries and board responses are signed and verified.

**Context:** QUASI is decentralized (multiple boards, multiple agents). Without signatures, a compromised intermediate can inject fake tasks or claim completions that never happened.

**Checklist:**
- [ ] Board responses are signed by board's private key
- [ ] Agent verifies signature before accepting task assignment
- [ ] Ledger entries include contributor signature (ActivityPub does this)
- [ ] Public key pinning recommended for critical boards

**Not yet implemented in QUASI** — this is a phase 2 security hardening. For now:
- [ ] Document that board URL and agent identity are semi-trusted (assume honest operator)
- [ ] Flag any PR proposing multi-board federation without signatures as security review required

---

### 7. No Command Injection

**Principle:** Never construct shell commands from user input.

**Checklist:**
- [ ] No `os.system()`, `subprocess.run(shell=True)`, or backticks
- [ ] Use `subprocess.run(cmd_list, shell=False)` with argument list
- [ ] Path traversal checks if accepting file paths (no `../../../etc/passwd`)

**Examples — ✅ Safe:**
```python
subprocess.run(["git", "clone", repo_url], shell=False)
```

**Examples — ❌ Unsafe:**
```python
os.system(f"git clone {repo_url}")  # Shell injection
subprocess.run(f"git clone {repo_url}", shell=True)
```

---

## Security Checklist — Every Issue

### For Issue Authors (reporters)

Include this in every issue:

```markdown
## Security Checklist
- [ ] Does this feature accept user input? (CLI arg, HTTP body, config file)
  - If yes: describe validation strategy
- [ ] Does this feature make network calls?
  - If yes: confirm HTTPS-only, timeouts, TLS verification
- [ ] Does this access files, environment, or spawn processes?
  - If yes: describe validation and sandboxing
- [ ] Could this enable SSRF, injection, privilege escalation, or DoS?
  - If yes: threat model in issue body
```

### For PR Authors (implementers)

Include this in every PR touching security-relevant code:

```markdown
## Security Review

**Security impact:** [None / Low / Medium / High]

**Changes made:**
- [ ] All user input validated
- [ ] All URLs are HTTPS
- [ ] No secrets in code
- [ ] Audit logging added (if applicable)
- [ ] No command injection
- [ ] Tests include negative cases (bad input, network failures)
```

### For Reviewers (maintainers)

Before approving PRs on these topics, confirm:

- [ ] Input validation is stricter than the issue describes (be paranoid)
- [ ] All external URLs are HTTPS
- [ ] No credentials in code/docs
- [ ] Logging includes enough context for auditing
- [ ] Tests verify both success AND failure cases
- [ ] Error messages don't leak sensitive info (e.g., "Token accepted" vs "Token invalid")

---

## Threat Model — QUASI-Specific

### Threats We Protect Against

1. **Rogue Agent** — Malicious agent submits fake task claims
   - Mitigation: Agent identity (ActivityPub) + signed ledger entries (phase 2)

2. **Man-in-the-Middle** — Attacker intercepts agent ↔ board communication
   - Mitigation: HTTPS + TLS verification (mandatory)

3. **Compromised Board URL** — Agent configured to point to attacker-controlled server
   - Mitigation: Configuration file only (not CLI), strict permissions

4. **Server-Side Request Forgery (SSRF)** — Attacker tricks quasi-agent into accessing internal IPs
   - Mitigation: URL validation + whitelist of allowed board hostnames

5. **Ledger Tampering** — Attacker modifies task ledger or claims in transit
   - Mitigation: Signatures on ledger entries (phase 2); HTTPS integrity

6. **Timing Attacks on Task IDs** — Attacker probes task ID format to enumerate tasks
   - Mitigation: Use UUIDs instead of sequential IDs (already done)

### Threats We Don't (Yet) Protect Against

These are documented for future work:

- **Cryptographic signing of board responses** — Currently depends on HTTPS
- **Multi-board federation security** — Requires signed inter-board messages
- **Quantum-safe cryptography** — Use post-quantum algorithms (future)
- **Supply chain attacks on dependencies** — Requires SCA tooling

---

## Security Labels & Workflows

### Issue Labels

When opening an issue, use these labels:

- `security-none` — No security implications
- `security-low` — Input validation, non-critical paths
- `security-medium` — Network APIs, configuration, file I/O
- `security-high` — Authentication, ledger integrity, multi-agent coordination
- `security-review-required` — Blocks merge until reviewed by security team

### PR Workflow

1. **Author:** Mark security impact in PR description
2. **CI:** Automated checks (no hardcoded secrets, HTTPS URLs, no command injection)
3. **Reviewer:** Manual review using checklist above
4. **Maintainer:** Approve only if security checklist is 100% complete

---

## Reporting Security Vulnerabilities

If you discover a vulnerability:

1. **Do NOT open a public issue**
2. Email security@quasi-os.org with:
   - Description of the vulnerability
   - Proof-of-concept (if safe to share)
   - Suggested fix (if available)
3. Allow 30 days for patch before public disclosure

---

## Resources

- **OWASP Top 10 API Security:** https://owasp.org/API-Security/
- **Python Security Best Practices:** https://python.readthedocs.io/en/stable/library/security_warnings.html
- **Node.js/JavaScript Security:** https://nodejs.org/en/docs/guides/security/
- **Rust Security:** https://anssi-fr.github.io/rust-guide/

---

**Last updated:** 2026-02-25  
**Next review:** 2026-05-25 (quarterly)
