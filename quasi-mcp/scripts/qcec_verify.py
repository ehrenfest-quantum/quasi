#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
"""QCEC equivalence checker — compares two OpenQASM 3.0 circuits."""

import json
import sys

def main():
    if len(sys.argv) != 3:
        print(json.dumps({"equivalent": False, "equivalence": "error", "error": "usage: qcec_verify.py <ref.qasm> <opt.qasm>"}))
        sys.exit(1)

    ref_path, opt_path = sys.argv[1], sys.argv[2]

    try:
        from mqt import qcec
        result = qcec.verify(ref_path, opt_path)
        equiv_str = str(result.equivalence)
        print(json.dumps({
            "equivalent": "equivalent" in equiv_str.lower() and "not" not in equiv_str.lower(),
            "equivalence": equiv_str,
            "error": None,
        }))
    except Exception as e:
        print(json.dumps({
            "equivalent": False,
            "equivalence": "error",
            "error": str(e),
        }))

if __name__ == "__main__":
    main()
