import { JobHandle, JobResult, SubmitCircuitInput } from "./types.js";

export * from "./types.js";

export interface HalClientOptions {
  baseUrl: string;
  fetchImpl?: typeof fetch;
}

class HalClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: HalClientOptions) {
    this.baseUrl = options.baseUrl.replace(/\/$/, "");
    this.fetchImpl = options.fetchImpl ?? fetch;
  }

  async submitCircuit(input: SubmitCircuitInput): Promise<JobHandle> {
    const response = await this.fetchImpl(`${this.baseUrl}/hal/jobs`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(input),
    });
    if (!response.ok) {
      throw new Error(`submitCircuit failed: ${response.status}`);
    }
    const data = (await response.json()) as JobHandle;
    return data;
  }

  async getResult(job: JobHandle): Promise<JobResult> {
    const response = await this.fetchImpl(`${this.baseUrl}/hal/jobs/${job.jobId}`, {
      method: "GET",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`getResult failed: ${response.status}`);
    }
    const data = (await response.json()) as JobResult;
    return data;
  }

  async listBackends(): Promise<string[]> {
    const response = await this.fetchImpl(`${this.baseUrl}/hal/backends`, {
      method: "GET",
      headers: { Accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`listBackends failed: ${response.status}`);
    }
    const data = (await response.json()) as { backends: string[] };
    return data.backends;
  }
}

let defaultClient = new HalClient({ baseUrl: "https://gawain.valiant-quantum.com" });

export function configureClient(options: HalClientOptions): void {
  defaultClient = new HalClient(options);
}

export async function submitCircuit(input: SubmitCircuitInput): Promise<JobHandle> {
  return defaultClient.submitCircuit(input);
}

export async function getResult(job: JobHandle): Promise<JobResult> {
  return defaultClient.getResult(job);
}

export async function listBackends(): Promise<string[]> {
  return defaultClient.listBackends();
}
