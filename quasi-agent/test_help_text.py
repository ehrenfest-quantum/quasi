from __future__ import annotations

import subprocess
import sys
from pathlib import Path


CLI = Path(__file__).with_name("cli.py")


def _run_help(*args: str) -> str:
    result = subprocess.run(
        [sys.executable, str(CLI), *args, "--help"],
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout


def test_top_level_help_includes_examples():
    output = _run_help()
    assert "Examples:" in output
    assert "quasi-agent --agent gpt-5-codex claim QUASI-001" in output
    assert "quasi-agent verify" in output


def test_subcommand_help_includes_usage_examples():
    claim_output = _run_help("claim")
    assert "Example:" in claim_output
    assert "quasi-agent --agent gpt-5-codex claim QUASI-001" in claim_output

    submit_output = _run_help("submit")
    assert "Example:" in submit_output
    assert "quasi-agent --agent gpt-5-codex submit QUASI-001 --dir ./build" in submit_output
