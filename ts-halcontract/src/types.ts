/**
 * A single-qubit decoherence channel, matching the Ehrenfest CBOR schema
 * (spec/ehrenfest-v0.1.cddl § NoiseChannel).
 *
 * type=1  Depolarizing        — uniform Pauli error probability `p`
 * type=2  AmplitudeDamping    — T1 energy-relaxation strength `gamma`
 * type=3  PhaseDamping        — pure T2* dephasing strength `gamma`
 */
export interface NoiseChannel {
  type: 1 | 2 | 3;
  qubit: number;
  /** Depolarizing error probability ∈ [0, 1]. Required when type=1. */
  p?: number;
  /** Kraus operator strength ∈ [0, 1]. Required when type=2 or type=3. */
  gamma?: number;
}

export interface SubmitCircuitInput {
  qasm: string;
  backend: string;
  shots: number;
  /**
   * Optional per-qubit noise model forwarded to the HAL driver.
   * Simulators use this to inject realistic decoherence; real-hardware
   * drivers may use it for error-mitigation strategy selection.
   * Channels correspond to those declared in the Ehrenfest program's
   * `noise_channels` field.
   */
  noiseChannels?: NoiseChannel[];
}

export interface JobHandle {
  jobId: string;
  backend: string;
}

export interface JobResult {
  status: "done" | "running" | "failed";
  counts?: Record<string, number>;
  jobId: string;
  backend: string;
  shots: number;
}
