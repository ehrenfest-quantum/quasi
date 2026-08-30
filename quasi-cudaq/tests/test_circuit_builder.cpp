// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// Unit tests for QuasiCircuitBuilder. No CUDA-Q installation required:
//
//   g++ -std=c++20 -I src tests/test_circuit_builder.cpp -o test_builder && ./test_builder

#include "QuasiCircuitBuilder.h"

#include <cstdio>
#include <string>

static int failures = 0;

static void check(bool cond, const std::string& what) {
    if (!cond) {
        std::printf("FAIL: %s\n", what.c_str());
        failures++;
    }
}

static void check_contains(const std::string& haystack,
                           const std::string& needle,
                           const std::string& what) {
    if (haystack.find(needle) == std::string::npos) {
        std::printf("FAIL: %s\n  expected to find: %s\n  in:\n%s\n",
                    what.c_str(), needle.c_str(), haystack.c_str());
        failures++;
    }
}

template <typename F>
static bool throws(F&& fn) {
    try {
        fn();
        return false;
    } catch (const std::runtime_error&) {
        return true;
    }
}

int main() {
    // ── GHZ state: h(0), cx(0,1), cx(1,2) ───────────────────────────────────
    {
        quasi::QuasiCircuitBuilder b;
        b.record("h", {}, {0}, {});
        b.record("x", {0}, {1}, {});
        b.record("x", {1}, {2}, {});
        check(b.num_qubits() == 3, "GHZ tracks 3 qubits");
        check(b.num_ops() == 3, "GHZ records 3 ops");

        std::string qasm = b.to_qasm({0, 1, 2});
        check_contains(qasm, "OPENQASM 2.0;", "QASM header");
        check_contains(qasm, "include \"qelib1.inc\";", "qelib include");
        check_contains(qasm, "qreg q[3];", "quantum register width");
        check_contains(qasm, "creg c[3];", "classical register width");
        check_contains(qasm, "h q[0];", "hadamard");
        check_contains(qasm, "cx q[0],q[1];", "first cnot");
        check_contains(qasm, "cx q[1],q[2];", "second cnot");
        check_contains(qasm, "measure q[2] -> c[2];", "measurement");
    }

    // ── Controlled variants map onto the supported controlled gates ────────
    {
        quasi::QuasiCircuitBuilder b;
        b.record("z", {0}, {1}, {});
        b.record("x", {0, 1}, {2}, {});
        std::string qasm = b.to_qasm({});
        check_contains(qasm, "cz q[0],q[1];", "cz");
        check_contains(qasm, "ccx q[0],q[1],q[2];", "ccx");
    }

    // ── Rotations carry their angle, and round-trip precisely ───────────────
    {
        quasi::QuasiCircuitBuilder b;
        b.record("rx", {}, {0}, {0.1});
        b.record("ry", {}, {1}, {0.25});
        std::string qasm = b.to_qasm({});
        check_contains(qasm, "ry(0.25) q[1];", "ry angle");
        // 0.1 is not exactly representable; the emitted literal must carry
        // enough digits to reproduce the same double.
        check_contains(qasm, "rx(0.10000000000000001) q[0];",
                       "angle full precision");
    }

    // ── Gates outside Huoma's set raise instead of emitting dropped QASM ────
    //
    // quasi-cudaq-ffi's QASM parser implements exactly
    //   h, x, y, z, s, t, rx, ry, rz, cx, cz, ccx, measure
    // so emitting anything else would be rejected downstream. Fail here,
    // where the offending gate can still be named.
    {
        quasi::QuasiCircuitBuilder b;
        check(throws([&] { b.record("swap", {}, {0, 1}, {}); }),
              "swap is unsupported and must raise");
        check(throws([&] { b.record("sdg", {}, {0}, {}); }),
              "sdg is unsupported and must raise");
        check(throws([&] { b.record("u3", {}, {0}, {0.1, 0.2, 0.3}); }),
              "u3 is unsupported and must raise");
        check(throws([&] { b.record("y", {0}, {1}, {}); }),
              "cy is unsupported and must raise");
        check(throws([&] { b.record("h", {0}, {1}, {}); }),
              "ch is unsupported and must raise");
        check(throws([&] { b.record("rz", {0}, {1}, {0.5}); }),
              "controlled-rz is unsupported and must raise");
        check(throws([&] { b.record("z", {0, 1}, {2}, {}); }),
              "ccz is unsupported and must raise");
        check(throws([&] { b.record("reset", {}, {0}, {}); }),
              "reset is unsupported and must raise");
        check(throws([&] { b.record("h", {}, {}, {}); }),
              "gate without target must raise");
        check(throws([&] { b.record("rx", {}, {0}, {}); }),
              "rotation without angle must raise");
        check(b.num_ops() == 0, "rejected gates are not recorded");
    }

    // ── Measured qubits widen the register ──────────────────────────────────
    {
        quasi::QuasiCircuitBuilder b;
        b.record("h", {}, {0}, {});
        std::string qasm = b.to_qasm({0, 4});
        check_contains(qasm, "qreg q[5];", "measured qubit widens register");
    }

    // ── ensure_qubits and reset ─────────────────────────────────────────────
    {
        quasi::QuasiCircuitBuilder b;
        b.record("h", {}, {0}, {});
        b.ensure_qubits(4);
        check(b.num_qubits() == 4, "ensure_qubits widens");
        b.ensure_qubits(2);
        check(b.num_qubits() == 4, "ensure_qubits never narrows");
        b.reset();
        check(b.num_ops() == 0 && b.num_qubits() == 0, "reset clears");
        check(throws([&] { b.to_qasm({}); }), "empty circuit must raise");
    }

    if (failures == 0) {
        std::printf("all QuasiCircuitBuilder tests passed\n");
        return 0;
    }
    std::printf("%d test(s) failed\n", failures);
    return 1;
}
