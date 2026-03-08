import re
import subprocess
import sys
from pathlib import Path


def test_export_creates_correct_qasm(tmp_path: Path):
    # Paths
    input_json = Path(__file__).parent / "fixtures" / "simple_spider.json"
    output_qasm = tmp_path / "out.qasm3"

    # Run the export module
    result = subprocess.run(
        [sys.executable, "-m", "afana.export", "--input", str(input_json), "--output", str(output_qasm)],
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, f"CLI failed: {result.stderr}"
    assert output_qasm.exists(), "Output QASM3 file was not created"

    content = output_qasm.read_text()
    # Check for rz line with any parameters on qubit 0
    assert re.search(r"rz\(.*\) q\[0\];", content), "Missing rz gate for Z‑spider"
    # Check for h line on qubit 1
    assert re.search(r"h q\[1\];", content), "Missing h gate for X‑spider"

    # Validate syntax with openqasm3.parser
    parse_cmd = [sys.executable, "-c", "import openqasm3.parser; openqasm3.parser.parse_file('" + str(output_qasm) + "')"]
    parse_result = subprocess.run(parse_cmd, capture_output=True, text=True)
    assert parse_result.returncode == 0, f"QASM3 syntax validation failed: {parse_result.stderr}"
