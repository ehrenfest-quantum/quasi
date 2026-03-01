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
   */
  noiseChannels?: NoiseChannel[];
  /**
   * Optional parameter bindings for parametric circuits (OpenQASM 3.0
   * `input float[64]` declarations).  Keys are parameter names; values
   * are concrete float values.  HAL drivers that do not support parametric
   * execution MUST return an error when this field is non-empty.
   *
   * Example: { "theta_0": 1.5707963, "theta_1": 0.7853981 }
   */
  parameters?: Record<string, number>;
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

/** Gate sets exposed by a backend (HAL Contract §4.2). */
export interface BackendGateSet {
  singleQubit: string[];
  twoQubit: string[];
  threeQubit: string[];
  native: string[];
}

/** Qubit connectivity graph (HAL Contract §4.3). */
export interface BackendTopology {
  kind: string;
  edges: [number, number][];
}

/** Device-wide noise averages (HAL Contract §4.4). All times in microseconds. */
export interface BackendNoiseProfile {
  t1?: number;
  t2?: number;
  singleQubitFidelity?: number;
  twoQubitFidelity?: number;
  readoutFidelity?: number;
  gateTime?: number;
}

/**
 * Full backend capabilities returned by GET /hal/backends/{name}.
 * Maps to HAL Contract §4.1 Capabilities.
 */
export interface BackendCapabilities {
  name: string;
  numQubits: number;
  gateSet: BackendGateSet;
  topology: BackendTopology;
  maxShots: number;
  maxCircuitOps?: number;
  isSimulator: boolean;
  features: string[];
  noiseProfile?: BackendNoiseProfile;
  isAvailable: boolean;
}
