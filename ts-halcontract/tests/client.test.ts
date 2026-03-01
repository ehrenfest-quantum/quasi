import { describe, expect, it } from "vitest";
import { configureClient, getResult, listBackends, submitCircuit } from "../src/index.js";

describe("@quasi/hal-contract", () => {
  it("submits a circuit and fetches result/backends", async () => {
    const fetchMock: typeof fetch = async (input: RequestInfo | URL): Promise<Response> => {
      const url = String(input);
      if (url.endsWith("/hal/jobs")) {
        return new Response(JSON.stringify({ jobId: "job-1", backend: "sim" }), { status: 200 });
      }
      if (url.endsWith("/hal/jobs/job-1")) {
        return new Response(
          JSON.stringify({
            status: "done",
            counts: { "00": 512, "11": 512 },
            jobId: "job-1",
            backend: "sim",
            shots: 1024
          }),
          { status: 200 }
        );
      }
      if (url.endsWith("/hal/backends")) {
        return new Response(JSON.stringify({ backends: ["sim", "ibm_torino", "iqm_garnet"] }), { status: 200 });
      }
      return new Response("not found", { status: 404 });
    };

    configureClient({ baseUrl: "https://hal.example.com", fetchImpl: fetchMock });

    const job = await submitCircuit({ qasm: "OPENQASM 2.0;", backend: "sim", shots: 1024 });
    expect(job).toEqual({ jobId: "job-1", backend: "sim" });

    const result = await getResult(job);
    expect(result.status).toBe("done");
    expect(result.counts).toEqual({ "00": 512, "11": 512 });

    const backends = await listBackends();
    expect(backends).toEqual(["sim", "ibm_torino", "iqm_garnet"]);
  });

  it("accepts noiseChannels in SubmitCircuitInput", async () => {
    let capturedBody: unknown;
    const fetchMock: typeof fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      capturedBody = JSON.parse(init?.body as string);
      return new Response(JSON.stringify({ jobId: "job-2", backend: "sim" }), { status: 200 });
    };

    configureClient({ baseUrl: "https://hal.example.com", fetchImpl: fetchMock });

    await submitCircuit({
      qasm: "OPENQASM 2.0;",
      backend: "sim",
      shots: 512,
      noiseChannels: [
        { type: 1, qubit: 0, p: 0.01 },
        { type: 2, qubit: 1, gamma: 0.001 },
      ],
    });

    expect((capturedBody as { noiseChannels: unknown[] }).noiseChannels).toHaveLength(2);
    expect((capturedBody as { noiseChannels: Array<{ type: number }> }).noiseChannels[0].type).toBe(1);
  });
});
