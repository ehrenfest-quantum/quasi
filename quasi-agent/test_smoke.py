import subprocess
import sys


def test_cli_help() -> None:
    subprocess.run(
        [sys.executable, "quasi-agent/cli.py", "--help"],
        check=True,
        stdout=subprocess.DEVNULL,
    )


def test_generate_issue_help() -> None:
    subprocess.run(
        [sys.executable, "quasi-agent/generate_issue.py", "--help"],
        check=True,
        stdout=subprocess.DEVNULL,
    )
