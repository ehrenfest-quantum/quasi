import argparse
import json
import sys
from pathlib import Path

def main() -> None:
    parser = argparse.ArgumentParser(description="Export ZX-IR JSON to OpenQASM 3")
    parser.add_argument("--input", required=True, help="Path to input JSON file containing spiders")
    parser.add_argument("--output", required=True, help="Path to write the generated QASM3 file")
    args = parser.parse_args()

    input_path = Path(args.input)
    output_path = Path(args.output)

    try:
        data = json.loads(input_path.read_text())
    except Exception as e:
        sys.stderr.write(f"Failed to read or parse input JSON: {e}\n")
        sys.exit(1)

    lines = []
    # Simple header for QASM3
    lines.append("OPENQASM 3.0;")
    lines.append("include \"stdgates.inc\";")
    lines.append("")

    # Determine number of qubits from the highest index in spiders
    max_qubit = -1
    for spider in data.get("spiders", []):
        for q in spider.get("qubits", []):
            if q > max_qubit:
                max_qubit = q
    if max_qubit >= 0:
        lines.append(f"qubit[{max_qubit + 1}] q;")
        lines.append("")

    for spider in data.get("spiders", []):
        typ = spider.get("type")
        qubits = spider.get("qubits", [])
        if not qubits:
            continue
        q = qubits[0]
        if typ == "Z":
            phase = spider.get("phase", 0)
            if phase != 0:
                # Use raw float; QASM3 accepts decimal literals.
                lines.append(f"rz({phase}) q[{q}];")
        elif typ == "X":
            lines.append(f"h q[{q}];")
        # H-box or other types could be added here.

    output_path.write_text("\n".join(lines) + "\n")

if __name__ == "__main__":
    main()
