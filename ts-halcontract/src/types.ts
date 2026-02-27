export interface SubmitCircuitInput {
  qasm: string;
  backend: string;
  shots: number;
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
