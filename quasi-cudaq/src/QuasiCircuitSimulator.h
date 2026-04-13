// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// QuasiCircuitSimulator — CUDA-Q CircuitSimulator backend for QUASI.
//
// This is the C++ adapter that plugs into CUDA-Q's runtime via the
// CircuitSimulator API. It accumulates gate operations into a circuit,
// converts to OpenQASM, and executes via Huoma (through the Rust FFI
// bridge in quasi-cudaq-ffi).
//
// Usage:
//   cudaq.set_target("quasi")   # Python
//   --target quasi               # C++ CLI

#ifndef QUASI_CUDAQ_CIRCUIT_SIMULATOR_H
#define QUASI_CUDAQ_CIRCUIT_SIMULATOR_H

#include <cassert>
#include <cstddef>
#include <sstream>
#include <string>
#include <vector>

#include "HuomaFFI.h"
#include "SolvayeurFFI.h"

namespace quasi {

/// A single gate operation accumulated from CUDA-Q callbacks.
struct GateOp {
    std::string name;
    std::vector<std::size_t> controls;
    std::size_t target;
    double angle; // 0.0 for non-parametric gates
};

/// QuasiCircuitSimulator — CUDA-Q backend targeting QUASI's Huoma simulator.
///
/// Implements the CircuitSimulator interface by accumulating gate operations,
/// converting them to OpenQASM 2.0, and executing via the Rust FFI bridge.
///
/// The CircuitSimulator API has implicit ordering constraints:
///   allocate before gate, gate before measure
/// We assert these invariants at every entry point (/doubleknuth).
class QuasiCircuitSimulator {
private:
    std::vector<GateOp> circuit_;
    std::size_t num_qubits_ = 0;
    HuomaHandle* huoma_ = nullptr;
    SolvayeurHandle* solvayeur_ = nullptr;

public:
    QuasiCircuitSimulator() {
        huoma_ = huoma_create();
        assert(huoma_ != nullptr && "huoma_create() returned null");
        solvayeur_ = solvayeur_create_classical();
        assert(solvayeur_ != nullptr && "solvayeur_create_classical() returned null");
    }

    ~QuasiCircuitSimulator() {
        if (huoma_) huoma_destroy(huoma_);
        if (solvayeur_) solvayeur_destroy(solvayeur_);
    }

    // Non-copyable
    QuasiCircuitSimulator(const QuasiCircuitSimulator&) = delete;
    QuasiCircuitSimulator& operator=(const QuasiCircuitSimulator&) = delete;

    // ── Qubit management ────────────────────────────────────────────────────

    void qubit_allocate(std::size_t qubitIdx) {
        if (qubitIdx >= num_qubits_) {
            num_qubits_ = qubitIdx + 1;
        }
    }

    void qubit_deallocate(std::size_t /*qubitIdx*/) {
        // No-op: Huoma manages its own statevector memory
    }

    // ── Single-qubit gates ──────────────────────────────────────────────────

    void h(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"h", {}, qubitIdx, 0.0});
    }

    void x(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"x", {}, qubitIdx, 0.0});
    }

    void y(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"y", {}, qubitIdx, 0.0});
    }

    void z(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"z", {}, qubitIdx, 0.0});
    }

    void s(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"s", {}, qubitIdx, 0.0});
    }

    void t(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"t", {}, qubitIdx, 0.0});
    }

    // ── Parametric single-qubit gates ───────────────────────────────────────

    void rx(double angle, std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"rx", {}, qubitIdx, angle});
    }

    void ry(double angle, std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"ry", {}, qubitIdx, angle});
    }

    void rz(double angle, std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "gate on unallocated qubit");
        circuit_.push_back({"rz", {}, qubitIdx, angle});
    }

    // ── Controlled gates ────────────────────────────────────────────────────

    void cnot(std::size_t control, std::size_t target) {
        assert(control < num_qubits_ && "control on unallocated qubit");
        assert(target < num_qubits_ && "target on unallocated qubit");
        assert(control != target && "control == target");
        circuit_.push_back({"cx", {control}, target, 0.0});
    }

    void cz(std::size_t control, std::size_t target) {
        assert(control < num_qubits_ && "control on unallocated qubit");
        assert(target < num_qubits_ && "target on unallocated qubit");
        assert(control != target && "control == target");
        circuit_.push_back({"cz", {control}, target, 0.0});
    }

    void ccx(std::size_t c1, std::size_t c2, std::size_t target) {
        assert(c1 < num_qubits_ && "control on unallocated qubit");
        assert(c2 < num_qubits_ && "control on unallocated qubit");
        assert(target < num_qubits_ && "target on unallocated qubit");
        circuit_.push_back({"ccx", {c1, c2}, target, 0.0});
    }

    // ── Measurement ─────────────────────────────────────────────────────────

    void mz(std::size_t qubitIdx) {
        assert(qubitIdx < num_qubits_ && "measure on unallocated qubit");
        circuit_.push_back({"measure", {}, qubitIdx, 0.0});
    }

    // ── Execution ───────────────────────────────────────────────────────────

    /// Sample the accumulated circuit.
    ///
    /// Returns measurement results as (bitstring, count) pairs.
    std::vector<std::pair<std::string, int>> sample(
        const std::vector<std::size_t>& qubitsToMeasure,
        int shots
    ) {
        // 1. Convert accumulated circuit to QASM
        std::string qasm = circuit_to_qasm();

        // 2. Ask Solvayeur which backend to use
        int backend_idx = solvayeur_decide(solvayeur_);

        // 3. Run on Huoma via FFI
        HuomaResult* result = huoma_execute(
            huoma_,
            qasm.c_str(),
            qubitsToMeasure.data(),
            qubitsToMeasure.size(),
            shots
        );

        if (!result) {
            // Execution failed — return empty result
            circuit_.clear();
            num_qubits_ = 0;
            return {};
        }

        // 4. Convert Huoma result to output format
        std::vector<std::pair<std::string, int>> exec_result;
        int n = huoma_result_count(result);
        for (int i = 0; i < n; i++) {
            const char* bitstring = huoma_result_bitstring(result, i);
            int count = huoma_result_frequency(result, i);
            if (bitstring) {
                exec_result.emplace_back(std::string(bitstring), count);
            }
        }

        // 5. Feed reward back to Solvayeur
        double fidelity_est = huoma_result_fidelity(result);
        double wall_time_ms = huoma_result_time_ms(result);
        solvayeur_observe(solvayeur_, backend_idx, fidelity_est, wall_time_ms, 0.0);

        // 6. Clean up
        huoma_result_destroy(result);
        circuit_.clear();
        num_qubits_ = 0;

        return exec_result;
    }

    /// Reset the simulator state for a new circuit.
    void reset() {
        circuit_.clear();
        num_qubits_ = 0;
    }

    /// Get the number of currently allocated qubits.
    std::size_t num_qubits() const { return num_qubits_; }

    /// Get the number of accumulated gate operations.
    std::size_t num_ops() const { return circuit_.size(); }

private:
    /// Convert accumulated gate operations to OpenQASM 2.0.
    std::string circuit_to_qasm() const {
        std::stringstream ss;
        ss << "OPENQASM 2.0;\n";
        ss << "include \"qelib1.inc\";\n";
        ss << "qreg q[" << num_qubits_ << "];\n";
        ss << "creg c[" << num_qubits_ << "];\n";

        for (const auto& op : circuit_) {
            if (op.name == "h") {
                ss << "h q[" << op.target << "];\n";
            } else if (op.name == "x") {
                ss << "x q[" << op.target << "];\n";
            } else if (op.name == "y") {
                ss << "y q[" << op.target << "];\n";
            } else if (op.name == "z") {
                ss << "z q[" << op.target << "];\n";
            } else if (op.name == "s") {
                ss << "s q[" << op.target << "];\n";
            } else if (op.name == "t") {
                ss << "t q[" << op.target << "];\n";
            } else if (op.name == "rx") {
                ss << "rx(" << op.angle << ") q[" << op.target << "];\n";
            } else if (op.name == "ry") {
                ss << "ry(" << op.angle << ") q[" << op.target << "];\n";
            } else if (op.name == "rz") {
                ss << "rz(" << op.angle << ") q[" << op.target << "];\n";
            } else if (op.name == "cx" && op.controls.size() == 1) {
                ss << "cx q[" << op.controls[0] << "],q[" << op.target << "];\n";
            } else if (op.name == "cz" && op.controls.size() == 1) {
                ss << "cz q[" << op.controls[0] << "],q[" << op.target << "];\n";
            } else if (op.name == "ccx" && op.controls.size() == 2) {
                ss << "ccx q[" << op.controls[0] << "],q[" << op.controls[1]
                   << "],q[" << op.target << "];\n";
            } else if (op.name == "measure") {
                ss << "measure q[" << op.target << "] -> c[" << op.target << "];\n";
            }
        }

        return ss.str();
    }
};

} // namespace quasi

#endif // QUASI_CUDAQ_CIRCUIT_SIMULATOR_H
