// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// QuasiCircuitBuilder — gate accumulation and OpenQASM 2.0 emission.
//
// Deliberately free of any CUDA-Q dependency so it can be unit-tested
// without a CUDA-Q installation. QuasiCircuitSimulator (which does depend
// on NVQIR headers) delegates all circuit construction here.

#ifndef QUASI_CUDAQ_CIRCUIT_BUILDER_H
#define QUASI_CUDAQ_CIRCUIT_BUILDER_H

#include <cstddef>
#include <iomanip>
#include <sstream>
#include <stdexcept>
#include <string>
#include <vector>

namespace quasi {

/// A single gate operation accumulated from CUDA-Q callbacks.
struct GateOp {
    std::string name;
    std::vector<std::size_t> controls;
    std::vector<std::size_t> targets;
    std::vector<double> parameters;
};

/// Accumulates gate operations and renders them as OpenQASM 2.0.
///
/// Only the gate set Huoma implements is emitted. Anything else raises
/// `std::runtime_error` at record time rather than producing QASM the
/// simulator would reject downstream.
class QuasiCircuitBuilder {
public:
    /// Record one gate operation.
    ///
    /// @param name       CUDA-Q operation name ("h", "x", "rx", ...)
    /// @param controls   Control qubit indices (may be empty)
    /// @param targets    Target qubit indices (exactly one)
    /// @param parameters Rotation angles (may be empty)
    /// @throws std::runtime_error if the (name, control count) pair is
    ///         outside the gate set Huoma supports.
    void record(const std::string& name,
                const std::vector<std::size_t>& controls,
                const std::vector<std::size_t>& targets,
                const std::vector<double>& parameters) {
        if (targets.empty()) {
            throw std::runtime_error(
                "QUASI backend: gate '" + name + "' has no target qubit");
        }
        // Validate up front so an unsupported gate fails where it was issued,
        // not later during QASM emission.
        (void)render(GateOp{name, controls, targets, parameters});

        for (auto q : controls) track(q);
        for (auto q : targets) track(q);
        circuit_.push_back(GateOp{name, controls, targets, parameters});
    }

    /// Drop the accumulated circuit and qubit count.
    void reset() {
        circuit_.clear();
        num_qubits_ = 0;
    }

    /// Ensure the register is at least `n` qubits wide.
    void ensure_qubits(std::size_t n) {
        if (n > num_qubits_) {
            num_qubits_ = n;
        }
    }

    /// Render the accumulated circuit as OpenQASM 2.0.
    ///
    /// @param measured Qubit indices to measure into the classical register.
    ///        Each qubit q is measured into c[q], so the classical register
    ///        index matches the qubit index.
    std::string to_qasm(const std::vector<std::size_t>& measured) const {
        std::size_t width = num_qubits_;
        for (auto q : measured) {
            if (q + 1 > width) {
                width = q + 1;
            }
        }
        if (width == 0) {
            throw std::runtime_error(
                "QUASI backend: cannot emit QASM for an empty circuit");
        }

        std::ostringstream ss;
        ss << "OPENQASM 2.0;\n";
        ss << "include \"qelib1.inc\";\n";
        ss << "qreg q[" << width << "];\n";
        ss << "creg c[" << width << "];\n";

        for (const auto& op : circuit_) {
            ss << render(op);
        }
        for (auto q : measured) {
            ss << "measure q[" << q << "] -> c[" << q << "];\n";
        }
        return ss.str();
    }

    std::size_t num_qubits() const { return num_qubits_; }
    std::size_t num_ops() const { return circuit_.size(); }

private:
    std::vector<GateOp> circuit_;
    std::size_t num_qubits_ = 0;

    void track(std::size_t qubitIdx) {
        if (qubitIdx + 1 > num_qubits_) {
            num_qubits_ = qubitIdx + 1;
        }
    }

    /// Format a double with enough precision to round-trip through QASM.
    static std::string angle(double value) {
        std::ostringstream ss;
        ss << std::setprecision(17) << value;
        return ss.str();
    }

    /// Render one operation as a QASM statement.
    ///
    /// The emitted gate set is deliberately limited to what Huoma's QASM
    /// parser (quasi-cudaq-ffi/src/qasm.rs) actually implements:
    ///
    ///     h, x, y, z, s, t, rx, ry, rz, cx, cz, ccx, measure
    ///
    /// Control qubits select the controlled variant: `x` with one control
    /// becomes `cx`, with two becomes `ccx`. Anything outside this set raises
    /// here, at the point the gate was issued, rather than producing QASM that
    /// Huoma would reject (or, worse, misread) further downstream.
    static std::string render(const GateOp& op) {
        std::ostringstream ss;
        const auto& c = op.controls;
        const auto& t = op.targets;
        const auto& p = op.parameters;

        auto q = [](std::size_t i) { return "q[" + std::to_string(i) + "]"; };

        auto unsupported = [&]() -> std::string {
            throw std::runtime_error(
                "QUASI backend: gate '" + op.name + "' with " +
                std::to_string(c.size()) + " control(s) and " +
                std::to_string(t.size()) +
                " target(s) is not in the gate set supported by the Huoma "
                "simulator (h, x, y, z, s, t, rx, ry, rz, cx, cz, ccx)");
        };

        if (t.size() != 1) return unsupported();
        const std::size_t tgt = t[0];

        if (c.empty()) {
            if (op.name == "h" || op.name == "x" || op.name == "y" ||
                op.name == "z" || op.name == "s" || op.name == "t") {
                ss << op.name << " " << q(tgt) << ";\n";
                return ss.str();
            }
            if (op.name == "rx" || op.name == "ry" || op.name == "rz") {
                if (p.size() != 1) {
                    throw std::runtime_error(
                        "QUASI backend: gate '" + op.name +
                        "' expects exactly 1 parameter, got " +
                        std::to_string(p.size()));
                }
                ss << op.name << "(" << angle(p[0]) << ") " << q(tgt) << ";\n";
                return ss.str();
            }
            return unsupported();
        }

        if (c.size() == 1) {
            if (op.name == "x") {
                ss << "cx " << q(c[0]) << "," << q(tgt) << ";\n";
                return ss.str();
            }
            if (op.name == "z") {
                ss << "cz " << q(c[0]) << "," << q(tgt) << ";\n";
                return ss.str();
            }
            return unsupported();
        }

        if (c.size() == 2 && op.name == "x") {
            ss << "ccx " << q(c[0]) << "," << q(c[1]) << "," << q(tgt) << ";\n";
            return ss.str();
        }

        return unsupported();
    }
};

} // namespace quasi

#endif // QUASI_CUDAQ_CIRCUIT_BUILDER_H
