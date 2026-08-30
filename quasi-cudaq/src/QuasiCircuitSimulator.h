// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2026 QUASI Contributors
//
// QuasiCircuitSimulator — NVQIR CircuitSimulator backend for QUASI.
//
// Subclasses nvqir::CircuitSimulatorBase<double> and is registered with
// NVQIR_REGISTER_SIMULATOR (see QuasiCircuitSimulator.cpp), which is what
// CUDA-Q's plugin loader actually looks for. Building this file requires
// CUDA-Q headers; the CUDA-Q-independent circuit construction lives in
// QuasiCircuitBuilder.h and is unit-tested separately.
//
// Usage:
//   cudaq.set_target("quasi")   # Python
//   nvq++ --target quasi        # C++
//
// Execution model: QUASI is a *batch* backend. Gates accumulate into a
// circuit, which is emitted as OpenQASM 2.0 and handed to Huoma (through
// the Rust FFI in quasi-cudaq-ffi) when CUDA-Q asks for samples. There is
// no live state vector on the C++ side, so state inspection and
// mid-circuit measurement are not offered — see the notes on those methods.

#ifndef QUASI_CUDAQ_CIRCUIT_SIMULATOR_H
#define QUASI_CUDAQ_CIRCUIT_SIMULATOR_H

#include "nvqir/CircuitSimulator.h"

#include <cstddef>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>

#include "HuomaFFI.h"
#include "QuasiCircuitBuilder.h"
#include "SolvayeurFFI.h"

namespace nvqir {

/// QuasiCircuitSimulator — routes CUDA-Q circuits through QUASI's Huoma
/// simulator via OpenQASM, and reports each execution back to the Solvayeur.
class QuasiCircuitSimulator : public nvqir::CircuitSimulatorBase<double> {
protected:
    /// Number of shots used to estimate an expectation value when CUDA-Q
    /// requests one without specifying a shot count. A sampling backend has
    /// no exact state vector to contract against, so <Z...Z> is estimated
    /// from counts. Statistical error is ~1/sqrt(kExpectationShots).
    static constexpr int kExpectationShots = 8192;

    quasi::QuasiCircuitBuilder builder_;
    HuomaHandle* huoma_ = nullptr;
    SolvayeurHandle* solvayeur_ = nullptr;

    /// No live state vector to grow — the circuit is replayed by Huoma.
    void addQubitToState() override {}

    /// QUASI cannot seed a circuit from caller-provided amplitudes.
    void addQubitsToState(std::size_t count,
                          const void* stateDataIn = nullptr) override {
        if (stateDataIn != nullptr) {
            throw std::runtime_error(
                "QUASI backend: state-vector initialization is not supported; "
                "prepare the state with gates instead.");
        }
        for (std::size_t i = 0; i < count; i++) {
            addQubitToState();
        }
    }

    void deallocateStateImpl() override { builder_.reset(); }

    void applyGate(const GateApplicationTask& task) override {
        // Qubit indices are passed through unchanged: getQubitOrdering() is
        // left at the base default (lsb), so the indices CUDA-Q hands us are
        // the indices we write into the QASM register.
        builder_.record(task.operationName, task.controls, task.targets,
                        task.parameters);
    }

    void setToZeroState() override { builder_.reset(); }

    /// Mid-circuit measurement requires collapsing a live state, which a
    /// batch backend has no way to do. Fail loudly rather than return a bit
    /// that carries no post-measurement state.
    bool measureQubit(const std::size_t qubitIdx) override {
        throw std::runtime_error(
            "QUASI backend: mid-circuit measurement of qubit " +
            std::to_string(qubitIdx) +
            " is not supported; measure at the end of the kernel and use "
            "cudaq::sample().");
    }

public:
    QuasiCircuitSimulator() {
        huoma_ = huoma_create();
        if (huoma_ == nullptr) {
            throw std::runtime_error(
                "QUASI backend: huoma_create() returned null");
        }
        solvayeur_ = solvayeur_create_classical();
        if (solvayeur_ == nullptr) {
            huoma_destroy(huoma_);
            huoma_ = nullptr;
            throw std::runtime_error(
                "QUASI backend: solvayeur_create_classical() returned null");
        }
        summaryData.name = name();
    }

    ~QuasiCircuitSimulator() override {
        if (huoma_) huoma_destroy(huoma_);
        if (solvayeur_) solvayeur_destroy(solvayeur_);
    }

    QuasiCircuitSimulator(const QuasiCircuitSimulator&) = delete;
    QuasiCircuitSimulator& operator=(const QuasiCircuitSimulator&) = delete;

    /// Huoma's QASM parser has no `reset` instruction, so a reset cannot be
    /// replayed in circuit order. Fail loudly rather than drop it: a silently
    /// ignored reset produces a circuit the caller did not write.
    void resetQubit(const std::size_t qubitIdx) override {
        throw std::runtime_error(
            "QUASI backend: reset of qubit " + std::to_string(qubitIdx) +
            " is not supported; allocate a fresh qubit instead.");
    }

    /// Sample the accumulated circuit on Huoma.
    cudaq::ExecutionResult sample(const std::vector<std::size_t>& qubits,
                                  const int shots,
                                  bool includeSequentialData = true) override {
        const bool expectationOnly = shots < 1;
        const int effectiveShots = expectationOnly ? kExpectationShots : shots;

        builder_.ensure_qubits(nQubitsAllocated);
        const std::string qasm = builder_.to_qasm(qubits);

        // Ask the Solvayeur which backend to run on. With the default
        // single-entry backend table this is always index 0; it becomes a
        // real choice once the backend table carries more than one resource.
        const int backendIdx = solvayeur_decide(solvayeur_);

        HuomaResult* result = huoma_execute(huoma_, qasm.c_str(), qubits.data(),
                                            qubits.size(), effectiveShots);
        if (result == nullptr) {
            throw std::runtime_error(
                "QUASI backend: huoma_execute() failed for a " +
                std::to_string(qubits.size()) + "-qubit measurement over " +
                std::to_string(effectiveShots) + " shots");
        }

        cudaq::ExecutionResult counts;
        double expVal = 0.0;
        const int n = huoma_result_count(result);
        for (int i = 0; i < n; i++) {
            const char* raw = huoma_result_bitstring(result, i);
            const int count = huoma_result_frequency(result, i);
            if (raw == nullptr) {
                continue;
            }
            const std::string bitstring(raw);
            if (bitstring.size() != qubits.size()) {
                huoma_result_destroy(result);
                throw std::runtime_error(
                    "QUASI backend: Huoma returned a " +
                    std::to_string(bitstring.size()) +
                    "-bit outcome for a " + std::to_string(qubits.size()) +
                    "-qubit measurement");
            }

            if (!expectationOnly) {
                if (includeSequentialData) {
                    counts.appendResult(bitstring, count);
                } else {
                    counts.counts[bitstring] += count;
                }
            }

            double p = static_cast<double>(count) /
                       static_cast<double>(effectiveShots);
            if (!cudaq::sample_result::has_even_parity(bitstring)) {
                p = -p;
            }
            expVal += p;
        }

        // Feed the observed execution back into the Solvayeur's bias fields.
        const double fidelity = huoma_result_fidelity(result);
        const double wallTimeMs = huoma_result_time_ms(result);
        huoma_result_destroy(result);
        solvayeur_observe(solvayeur_, backendIdx, fidelity, wallTimeMs, 0.0);

        if (expectationOnly) {
            return cudaq::ExecutionResult{{}, expVal};
        }
        counts.expectationValue = expVal;
        return counts;
    }

    /// QUASI holds no caller-visible state vector: circuits are replayed by
    /// Huoma on demand rather than kept live on the C++ side.
    std::unique_ptr<cudaq::SimulationState>
    createStateFromData(const cudaq::state_data&) override {
        throw std::runtime_error(
            "QUASI backend: constructing a simulator state from raw data is "
            "not supported.");
    }

    std::string name() const override { return "quasi"; }

    NVQIR_SIMULATOR_CLONE_IMPL(QuasiCircuitSimulator)
};

} // namespace nvqir

#endif // QUASI_CUDAQ_CIRCUIT_SIMULATOR_H
