#!/usr/bin/env python3
"""
QUASI Senate Watchdog
=====================
AI-powered watchdog for the quasi-senate systemd services.
Uses Claude Sonnet for diagnosis and repair decisions.
Only calls the API when something is wrong or stalled.

Usage:
    python3 senate_watchdog.py                         # One-shot check
    python3 senate_watchdog.py --daemon --interval 300 # Continuous
    python3 senate_watchdog.py --dry-run               # Diagnose only

Env vars (loaded from /home/vops/.env.quasi + system profile):
    ANTHROPIC_API_KEY     - Required for AI diagnosis
    DATABASE_URL          - Postgres for telemetry checks
    MATRIX_BOT_TOKEN      - Optional Matrix notifications
    MATRIX_ROOM_ID        - Optional Matrix room
    MATRIX_HOMESERVER     - Optional Matrix homeserver URL
"""

import argparse
import json
import logging
import os
import re
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone
from pathlib import Path


# ── Config ────────────────────────────────────────────────────────────────────

SERVICES = {
    "draft":   "quasi-senate-draft.service",
    "solve":   "quasi-senate-solve.service",
    "council": "quasi-senate-council.service",
}

LOG_FILES = {
    "draft":   Path("/home/vops/logs/quasi-senate-draft.log"),
    "solve":   Path("/home/vops/logs/quasi-senate-solve.log"),
    "council": Path("/home/vops/logs/quasi-senate-council.log"),
}

STATE_FILE = Path("/home/vops/quasi-senate-state.json")

# A service in `activating` longer than these thresholds is considered stalled
STALL_THRESHOLDS_SEC = {
    "draft":   7200,   # 2h — drafting + gate review can be long
    "solve":   5400,   # 90m — solver + reviewer
    "council": 3600,   # 1h — council call is usually fast
}

# Timer expected intervals (used to detect if timers have stopped firing)
TIMER_INTERVALS_SEC = {
    "draft":   7200,   # every 2h
    "solve":   7200,   # every 2h
    "council": 86400,  # daily
}

CONFIG = {
    "api_key":         os.environ.get("ANTHROPIC_API_KEY", ""),
    "model":           "claude-sonnet-4-6",
    "max_tokens":      2048,
    "agent_timeout":   180,
    "max_restarts_per_hour": 4,
    "state_dir":       Path("/var/lib/quasi-senate-watchdog"),
    "log_file":        Path("/var/log/quasi-senate-watchdog.log"),
    "action_log":      Path("/var/log/quasi-senate-watchdog-actions.jsonl"),
    "database_url":    os.environ.get("DATABASE_URL", ""),
    "matrix_token":    os.environ.get("MATRIX_BOT_TOKEN",
                           "rduSajKuSaXXWj6HqVS8eSQMrh4Jm4nF"),
    "matrix_room":     os.environ.get("MATRIX_ROOM_ID",
                           "!CerauaaS111HsAzJXI:gawain.valiant-quantum.com"),
    "matrix_hs":       os.environ.get("MATRIX_HOMESERVER",
                           "https://gawain.valiant-quantum.com"),
}


# ── Logging ───────────────────────────────────────────────────────────────────

log = logging.getLogger("senate-watchdog")
log.setLevel(logging.INFO)
_fmt = logging.Formatter("%(asctime)s [%(levelname)s] %(message)s")
_sh = logging.StreamHandler()
_sh.setFormatter(_fmt)
log.addHandler(_sh)
try:
    CONFIG["state_dir"].mkdir(parents=True, exist_ok=True)
    _fh = logging.FileHandler(CONFIG["log_file"])
    _fh.setFormatter(_fmt)
    log.addHandler(_fh)
except PermissionError:
    log.warning(f"Cannot write to {CONFIG['log_file']}, file logging disabled")


def log_action(action: dict):
    action["timestamp"] = datetime.now(timezone.utc).isoformat()
    try:
        with open(CONFIG["action_log"], "a") as f:
            f.write(json.dumps(action) + "\n")
    except OSError as e:
        log.warning(f"Failed to write action log: {e}")


# ── Matrix alerts ─────────────────────────────────────────────────────────────

def send_matrix(message: str):
    """Send a Matrix message to Daniel's room."""
    import urllib.request
    import urllib.parse
    token = CONFIG["matrix_token"]
    room = CONFIG["matrix_room"]
    hs = CONFIG["matrix_hs"]
    if not token or not room:
        return
    txn = str(int(time.time() * 1000))
    room_encoded = urllib.parse.quote(room, safe="")
    url = f"{hs}/_matrix/client/v3/rooms/{room_encoded}/send/m.room.message/{txn}"
    body = json.dumps({"msgtype": "m.text", "body": message}).encode()
    req = urllib.request.Request(
        url, data=body, method="PUT",
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    )
    try:
        urllib.request.urlopen(req, timeout=10)
    except Exception as e:
        log.warning(f"Matrix alert failed: {e}")


# ── Shell ─────────────────────────────────────────────────────────────────────

def run(cmd: str, timeout: int = 30) -> dict:
    try:
        r = subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=timeout)
        return {"stdout": r.stdout.strip(), "stderr": r.stderr.strip(), "rc": r.returncode}
    except subprocess.TimeoutExpired:
        return {"stdout": "", "stderr": f"Timeout ({timeout}s)", "rc": -1}
    except Exception as e:
        return {"stdout": "", "stderr": str(e), "rc": -1}


# ── Health Checks ─────────────────────────────────────────────────────────────

def check_services() -> dict:
    """Check systemd status for each senate service."""
    results = {}
    for name, unit in SERVICES.items():
        r = run(f"systemctl is-active {unit}")
        state = r["stdout"].strip()

        # Get ActiveEnterTimestamp for stall detection
        r2 = run(f"systemctl show {unit} --property=ActiveEnterTimestamp,SubState --value")
        props = {}
        for line in r2["stdout"].splitlines():
            if "=" in line:
                k, v = line.split("=", 1)
                props[k] = v
        # ActiveEnterTimestamp in systemd format: "Sun 2026-03-01 13:31:52 UTC"
        enter_ts = props.get("ActiveEnterTimestamp", "")
        sub_state = props.get("SubState", state)

        activating_for_sec = None
        if enter_ts:
            try:
                ts = datetime.strptime(enter_ts, "%a %Y-%m-%d %H:%M:%S %Z")
                ts = ts.replace(tzinfo=timezone.utc)
                activating_for_sec = int((datetime.now(timezone.utc) - ts).total_seconds())
            except ValueError:
                pass

        stalled = False
        if state == "activating" and activating_for_sec is not None:
            stalled = activating_for_sec > STALL_THRESHOLDS_SEC.get(name, 3600)

        results[name] = {
            "unit": unit,
            "state": state,
            "sub_state": sub_state,
            "activating_for_sec": activating_for_sec,
            "stalled": stalled,
            "healthy": state in ("active", "activating", "inactive") and not stalled,
        }

    return results


def check_timers() -> dict:
    """Check that senate timers have fired recently."""
    results = {}
    for name in SERVICES:
        timer = f"quasi-senate-{name}.timer"
        r = run(f"systemctl show {timer} --property=LastTriggerUSec --value")
        last_usec = r["stdout"].strip()
        if not last_usec or last_usec == "0":
            results[name] = {"healthy": False, "reason": "timer never fired"}
            continue
        try:
            last_sec = int(last_usec) / 1_000_000
            age = time.time() - last_sec
            threshold = TIMER_INTERVALS_SEC.get(name, 86400) * 2.5
            results[name] = {
                "healthy": age < threshold,
                "last_fired_ago_sec": int(age),
                "threshold_sec": int(threshold),
            }
        except ValueError:
            results[name] = {"healthy": True, "raw": last_usec}
    return results


def check_log_errors() -> dict:
    """Scan log files for recent error patterns."""
    results = {}
    for name, path in LOG_FILES.items():
        if not path.exists():
            results[name] = {"healthy": True, "note": "log file absent (no runs yet)"}
            continue
        try:
            text = path.read_text(errors="replace")
            lines = text.splitlines()
            errors = [l for l in lines if "Error:" in l or "error" in l.lower()]
            results[name] = {
                "healthy": True,
                "total_lines": len(lines),
                "error_lines": len(errors),
                "last_errors": errors[-5:] if errors else [],
            }
        except OSError as e:
            results[name] = {"healthy": False, "reason": str(e)}
    return results


def check_state_permissions() -> dict:
    """Verify the state and lock files are owned by vops."""
    issues = []
    for path in [STATE_FILE, Path(str(STATE_FILE) + ".lock")]:
        if not path.exists():
            continue
        r = run(f"stat -c '%U %G' {path}")
        owner = r["stdout"].strip()
        if owner and not owner.startswith("vops"):
            issues.append(f"{path}: owned by {owner!r} (should be vops)")
    return {"healthy": len(issues) == 0, "issues": issues}


def check_telemetry_activity() -> dict:
    """Check if senate has written telemetry rows in the last 6 hours."""
    db = CONFIG["database_url"]
    if not db:
        return {"healthy": True, "note": "DATABASE_URL not set"}
    sql = "SELECT COUNT(*) FROM senate_telemetry WHERE timestamp > NOW() - INTERVAL '6 hours'"
    r = run(f"psql '{db}' -t -c \"{sql}\"", timeout=10)
    if r["rc"] != 0:
        return {"healthy": False, "reason": f"DB error: {r['stderr'][:200]}"}
    try:
        count = int(r["stdout"].strip())
        return {"healthy": True, "rows_last_6h": count}
    except ValueError:
        return {"healthy": False, "reason": "Failed to parse telemetry count"}


# ── Diagnostics ───────────────────────────────────────────────────────────────

_SECRET_PATTERNS = [
    (re.compile(r"(ANTHROPIC_API_KEY\s*=\s*)\S+"), r"\1[REDACTED]"),
    (re.compile(r"(Bearer\s+)\S+", re.IGNORECASE), r"\1[REDACTED]"),
    (re.compile(r"sk-ant-[A-Za-z0-9\-_]+"), "[REDACTED_ANTHROPIC_KEY]"),
    (re.compile(r"postgres://[^@]+@"), "postgres://[REDACTED]@"),
    (re.compile(r"(?<=[=: ])[A-Za-z0-9+/]{64,}={0,2}(?=\s|$)"), "[REDACTED_SECRET]"),
]


def sanitize(text: str) -> str:
    for pattern, replacement in _SECRET_PATTERNS:
        text = pattern.sub(replacement, text)
    return text


def gather_diagnostics(failed_checks: dict) -> str:
    parts = []

    # Service status
    for name, unit in SERVICES.items():
        r = run(f"systemctl status {unit} --no-pager -l 2>&1 | head -20")
        parts.append(f"### systemctl status {name}\n```\n{r['stdout']}\n```")

    # Relevant log tails
    for name, path in LOG_FILES.items():
        if path.exists():
            r = run(f"tail -30 {path}")
            parts.append(f"### {name} log (last 30 lines)\n```\n{r['stdout']}\n```")

    # State file
    if STATE_FILE.exists():
        r = run(f"cat {STATE_FILE}")
        parts.append(f"### state file\n```json\n{r['stdout']}\n```")

    # State file permissions
    r = run(f"ls -la {STATE_FILE} {STATE_FILE}.lock 2>/dev/null")
    parts.append(f"### state file permissions\n```\n{r['stdout']}\n```")

    # Telemetry summary
    db = CONFIG["database_url"]
    if db:
        sql = "SELECT role, COUNT(*), MAX(timestamp) FROM senate_telemetry GROUP BY role ORDER BY role"
        r = run(f"psql '{db}' -t -c \"{sql}\" 2>&1", timeout=10)
        parts.append(f"### telemetry summary\n```\n{r['stdout'] or r['stderr']}\n```")

    # Timer next run
    r = run("systemctl list-timers quasi-senate* --no-pager 2>&1")
    parts.append(f"### timers\n```\n{r['stdout']}\n```")

    # Failed checks summary
    parts.append(f"### failed_checks\n```json\n{json.dumps(failed_checks, indent=2)}\n```")

    raw = "\n\n".join(parts)
    return sanitize(raw)


# ── Repair Actions ────────────────────────────────────────────────────────────

ACTIONS = {
    "restart_service": {
        "desc": "Restart a quasi-senate systemd service",
        "params": ["service"],
    },
    "get_logs": {
        "desc": "Get the last 50 lines of a senate log file",
        "params": ["service"],
    },
    "get_service_status": {
        "desc": "Get full systemctl status for a service",
        "params": ["service"],
    },
    "fix_state_permissions": {
        "desc": "Fix ownership of state and lock files (chown vops:vops)",
        "params": [],
    },
    "get_state_file": {
        "desc": "Read the current senate state file",
        "params": [],
    },
    "get_telemetry_recent": {
        "desc": "Query the 10 most recent senate_telemetry rows",
        "params": [],
    },
    "reset_solve_retries": {
        "desc": "Clear solve_retries in the state file so stuck issues are retried",
        "params": [],
    },
}

SERVICE_NAMES = list(SERVICES.keys())


def build_tools() -> list:
    tools = []
    for name, a in ACTIONS.items():
        props: dict = {}
        req: list = []
        if "service" in a["params"]:
            props["service"] = {
                "type": "string",
                "enum": SERVICE_NAMES,
                "description": "Senate service: draft, solve, or council",
            }
            req.append("service")
        tools.append({
            "name": name,
            "description": a["desc"],
            "input_schema": {"type": "object", "properties": props, "required": req},
        })
    return tools


def execute_action(name: str, params: dict, dry_run: bool = False) -> str:
    log_action({"action": name, "params": params, "dry_run": dry_run})

    if name == "restart_service":
        svc = params.get("service", "")
        if svc not in SERVICE_NAMES:
            return f"BLOCKED: unknown service '{svc}'"
        unit = SERVICES[svc]
        if dry_run:
            return f"[DRY RUN] systemctl restart {unit}"
        r = run(f"systemctl restart {unit}", timeout=30)
        log_action({"action": name, "rc": r["rc"], "out": r["stdout"][:300]})
        return f"Exit {r['rc']}\n{r['stdout'] or r['stderr']}"

    elif name == "get_logs":
        svc = params.get("service", "")
        path = LOG_FILES.get(svc)
        if not path:
            return f"Unknown service '{svc}'"
        if not path.exists():
            return "Log file not found"
        r = run(f"tail -50 {path}")
        return r["stdout"]

    elif name == "get_service_status":
        svc = params.get("service", "")
        unit = SERVICES.get(svc, "")
        if not unit:
            return f"Unknown service '{svc}'"
        r = run(f"systemctl status {unit} --no-pager -l 2>&1 | head -40")
        return r["stdout"]

    elif name == "fix_state_permissions":
        if dry_run:
            return f"[DRY RUN] chown vops:vops {STATE_FILE} {STATE_FILE}.lock"
        targets = f"{STATE_FILE} {STATE_FILE}.lock"
        r = run(f"chown vops:vops {targets} 2>/dev/null; ls -la {targets}")
        log_action({"action": name, "rc": r["rc"], "out": r["stdout"][:300]})
        return r["stdout"] or r["stderr"]

    elif name == "get_state_file":
        if not STATE_FILE.exists():
            return "State file not found"
        return sanitize(STATE_FILE.read_text(errors="replace"))

    elif name == "get_telemetry_recent":
        db = CONFIG["database_url"]
        if not db:
            return "DATABASE_URL not configured"
        sql = "SELECT id, role, model_id, downstream_verdict, timestamp FROM senate_telemetry ORDER BY id DESC LIMIT 10"
        r = run(f"psql '{db}' -c \"{sql}\" 2>&1", timeout=10)
        return sanitize(r["stdout"] or r["stderr"])

    elif name == "reset_solve_retries":
        if not STATE_FILE.exists():
            return "State file not found"
        if dry_run:
            return "[DRY RUN] Would clear solve_retries in state file"
        try:
            state = json.loads(STATE_FILE.read_text())
            state["solve_retries"] = {}
            STATE_FILE.write_text(json.dumps(state, indent=2))
            log_action({"action": name, "result": "cleared"})
            return "solve_retries cleared"
        except Exception as e:
            return f"Error: {e}"

    return f"Unknown action: {name}"


# ── System prompt ─────────────────────────────────────────────────────────────

SYSTEM_PROMPT = """You are the QUASI Senate watchdog agent. You monitor and repair the quasi-senate
Rust governance daemon running as three systemd services on Camelot (87.106.219.154).

## Architecture
The senate loop runs three services as systemd timers:
- **quasi-senate-draft** (every 2h): A2 drafter picks open GitHub issues, writes a solution draft,
  sends to A3 gate for quality review. Long LLM calls are normal — can run 30-90 minutes.
- **quasi-senate-solve** (every 2h): B1 solver generates code edits from accepted drafts, B2 reviewer
  checks them. Creates GitHub PRs when accepted.
- **quasi-senate-council** (daily 06:00): A1 council generates the Phase Charter (priorities, quotas).

## State
- State file: /home/vops/quasi-senate-state.json — tracks charter, retries, phase counts
- State must be owned by vops:vops (root ownership = Permission denied crash)
- Log files: /home/vops/logs/quasi-senate-{draft,solve,council}.log
- Telemetry: postgres senate_telemetry table

## Common problems and fixes
1. **Permission denied on state file** → fix_state_permissions
2. **Service in activating > threshold (stalled)** → get_logs first, then restart_service
3. **Solve retries exhausted for one issue** → reset_solve_retries (state will pick next issue)
4. **Service failed (inactive/failed)** → get_service_status + get_logs, then restart_service
5. **No telemetry rows in 6h** → likely a stall or failure; check logs first

## Rules
- NEVER restart council unless it has been activating > 2h or is in failed state
- Always call get_logs before restarting a stalled service — the stall might be a normal long LLM call
- Draft and solve can legitimately run for 30-90 minutes — only restart if > threshold
- Be concise: state root cause, action taken, result"""


# ── Claude agent loop ─────────────────────────────────────────────────────────

def call_claude(messages: list, tools: list) -> dict:
    import urllib.request

    body = {
        "model": CONFIG["model"],
        "max_tokens": CONFIG["max_tokens"],
        "system": SYSTEM_PROMPT,
        "messages": messages,
        "tools": tools,
    }
    req = urllib.request.Request(
        "https://api.anthropic.com/v1/messages",
        data=json.dumps(body).encode(),
        headers={
            "Content-Type": "application/json",
            "x-api-key": CONFIG["api_key"],
            "anthropic-version": "2023-06-01",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return json.loads(resp.read())
    except Exception as e:
        log.error(f"Claude API failed: {e}")
        return {"error": str(e)}


def run_agent_loop(diagnostics: str, dry_run: bool = False) -> str:
    messages = [{"role": "user", "content":
        f"Senate health check failed. Diagnostics:\n\n{diagnostics}\n\n"
        "Investigate with get_logs first, then apply the minimal fix needed."}]
    tools = build_tools()
    deadline = time.monotonic() + CONFIG["agent_timeout"]

    for i in range(6):
        if time.monotonic() > deadline:
            return "Agent loop timed out"

        resp = call_claude(messages, tools)
        if "error" in resp:
            return f"Agent error: {resp['error']}"

        content = resp.get("content", [])
        messages.append({"role": "assistant", "content": content})

        tool_uses = [b for b in content if b.get("type") == "tool_use"]
        texts = [b.get("text", "") for b in content if b.get("type") == "text"]

        if not tool_uses:
            return "\n".join(texts)

        results = []
        for tu in tool_uses:
            log.info(f"Agent [{i+1}/6]: {tu['name']}({tu.get('input', {})})")
            out = execute_action(tu["name"], tu.get("input", {}), dry_run=dry_run)
            results.append({
                "type": "tool_result",
                "tool_use_id": tu["id"],
                "content": out[:2000],
            })
        messages.append({"role": "user", "content": results})

    return "Agent completed 6 iterations — manual check may be needed."


# ── Rate limiting ─────────────────────────────────────────────────────────────

def _restart_log_path() -> Path:
    return CONFIG["state_dir"] / "restart-history.json"


def restart_budget_ok() -> bool:
    now = time.time()
    path = _restart_log_path()
    try:
        restarts = json.loads(path.read_text())
    except (FileNotFoundError, json.JSONDecodeError, OSError):
        restarts = []
    restarts = [t for t in restarts if now - t < 3600]
    try:
        path.write_text(json.dumps(restarts))
    except OSError:
        pass
    return len(restarts) < CONFIG["max_restarts_per_hour"]


def record_restart():
    path = _restart_log_path()
    try:
        restarts = json.loads(path.read_text())
    except (FileNotFoundError, json.JSONDecodeError, OSError):
        restarts = []
    restarts.append(time.time())
    try:
        path.write_text(json.dumps(restarts))
    except OSError:
        pass


# ── Main health check ─────────────────────────────────────────────────────────

_check_lock = threading.Lock()


def run_health_check(dry_run: bool = False) -> bool:
    if not _check_lock.acquire(blocking=False):
        log.info("Previous check still running, skipping")
        return True
    try:
        return _run_health_check_inner(dry_run)
    finally:
        _check_lock.release()


def _run_health_check_inner(dry_run: bool = False) -> bool:
    log.info("=" * 60)
    log.info("QUASI Senate health check")

    checks = {
        "services":     check_services(),
        "timers":       check_timers(),
        "log_errors":   check_log_errors(),
        "permissions":  check_state_permissions(),
        "telemetry":    check_telemetry_activity(),
    }

    # Determine if anything is actually wrong
    failed: dict = {}

    # Services: failed or stalled
    svc_check = checks["services"]
    for name, info in svc_check.items():
        if not info.get("healthy", True):
            failed.setdefault("services", {})[name] = info

    # Timers: stopped firing (skip council — daily is fine)
    timer_check = checks["timers"]
    for name, info in timer_check.items():
        if name == "council":
            continue
        if not info.get("healthy", True):
            failed.setdefault("timers", {})[name] = info

    # Permissions
    perm_check = checks["permissions"]
    if not perm_check.get("healthy", True):
        failed["permissions"] = perm_check

    # Telemetry silence (only report, not block)
    tel = checks["telemetry"]
    rows = tel.get("rows_last_6h", None)
    if rows is not None and rows == 0:
        failed["telemetry"] = {"warning": "no rows in last 6h", **tel}

    if not failed:
        # Log a brief summary
        svc_states = {n: i["state"] for n, i in svc_check.items()}
        tel_rows = checks["telemetry"].get("rows_last_6h", "?")
        log.info(f"All healthy — services: {svc_states}, telemetry_6h: {tel_rows}")
        return True

    log.warning(f"Issues detected: {list(failed.keys())}")

    if not restart_budget_ok():
        msg = "Restart budget exhausted (4/hour) — manual intervention needed"
        log.error(msg)
        send_matrix(f"[QUASI Senate Watchdog] {msg}")
        return False

    if not CONFIG["api_key"]:
        log.error("No ANTHROPIC_API_KEY — cannot run agent, attempting blind fix")
        # Only auto-fix permission issues without AI
        if "permissions" in failed:
            if not dry_run:
                run(f"chown vops:vops {STATE_FILE} {STATE_FILE}.lock 2>/dev/null")
                log.info("Blind fix: chowned state files")
        return False

    diag = gather_diagnostics(failed)
    log.info(f"Calling Claude Sonnet for diagnosis ({len(diag)} chars)...")
    result = run_agent_loop(diag, dry_run=dry_run)
    log.info(f"Agent result:\n{result}")
    record_restart()

    # Notify via Matrix
    summary = result[:600] if result else "No summary"
    send_matrix(
        f"[QUASI Senate Watchdog]\n"
        f"Issues: {', '.join(failed.keys())}\n\n"
        f"Agent: {summary}"
    )

    return False


def main():
    p = argparse.ArgumentParser(description="QUASI Senate Watchdog")
    p.add_argument("--dry-run", action="store_true", help="Diagnose only, don't execute repairs")
    p.add_argument("--daemon", action="store_true", help="Run continuously")
    p.add_argument("--interval", type=int, default=300, help="Check interval in seconds (default: 300)")
    args = p.parse_args()

    if args.daemon:
        log.info(f"Daemon mode (every {args.interval}s)")
        while True:
            try:
                run_health_check(dry_run=args.dry_run)
            except Exception as e:
                log.error(f"Check crashed: {e}", exc_info=True)
                send_matrix(f"[QUASI Senate Watchdog] Crash: {e}")
            time.sleep(args.interval)
    else:
        ok = run_health_check(dry_run=args.dry_run)
        sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
