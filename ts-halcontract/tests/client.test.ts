import { describe, expect, it } from "vitest";
import { configureClient, getBackendCapabilities, getResult, listBackends, submitCircuit } from "../src/index.js";

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

  it("accepts parameters in SubmitCircuitInput for parametric circuits", async () => {
    let capturedBody: unknown;
    const fetchMock: typeof fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
      capturedBody = JSON.parse(init?.body as string);
      return new Response(JSON.stringify({ jobId: "job-3", backend: "sim" }), { status: 200 });
    };

    configureClient({ baseUrl: "https://hal.example.com", fetchImpl: fetchMock });

    await submitCircuit({
      qasm: "OPENQASM 3.0; input float[64] theta_0; input float[64] theta_1;",
      backend: "sim",
      shots: 1024,
      parameters: { theta_0: 1.5707963, theta_1: 0.7853981 },
    });

    const body = capturedBody as { parameters: Record<string, number> };
    expect(body.parameters).toBeDefined();
    expect(body.parameters["theta_0"]).toBeCloseTo(1.5707963);
    expect(body.parameters["theta_1"]).toBeCloseTo(0.7853981);
  });

  it("fetches backend capabilities from GET /hal/backends/{name}", async () => {
    const capabilitiesFixture = {
      name: "ibm_torino",
      numQubits: 156,
      gateSet: {
        singleQubit: ["rz", "sx", "x"],
        twoQubit: ["cz", "ecr"],
        threeQubit: [],
        native: ["rz", "sx", "x", "cz"],
      },
      topology: { kind: "HeavyHex", edges: [[0, 1], [1, 2]] },
      maxShots: 300000,
      isSimulator: false,
      features: ["dynamic_circuits", "mid_circuit_measurement"],
      noiseProfile: { t1: 150.0, t2: 80.0, singleQubitFidelity: 0.9998 },
      isAvailable: true,
    };

    const fetchMock: typeof fetch = async (input: RequestInfo | URL): Promise<Response> => {
      const url = String(input);
      if (url.endsWith("/hal/backends/ibm_torino")) {
        return new Response(JSON.stringify(capabilitiesFixture), { status: 200 });
      }
      return new Response("not found", { status: 404 });
    };

    configureClient({ baseUrl: "https://hal.example.com", fetchImpl: fetchMock });

    const caps = await getBackendCapabilities("ibm_torino");
    expect(caps.name).toBe("ibm_torino");
    expect(caps.numQubits).toBe(156);
    expect(caps.isSimulator).toBe(false);
    expect(caps.gateSet.native).toContain("rz");
    expect(caps.noiseProfile?.t1).toBe(150.0);
    expect(caps.features).toContain("dynamic_circuits");
  });

  it("encodes backend name in getBackendCapabilities URL", async () => {
    let capturedUrl: string = "";
    const fetchMock: typeof fetch = async (input: RequestInfo | URL): Promise<Response> => {
      capturedUrl = String(input);
      return new Response(
        JSON.stringify({
          name: "iqm/garnet",
          numQubits: 20,
          gateSet: { singleQubit: ["prx"], twoQubit: ["cz"], threeQubit: [], native: ["prx", "cz"] },
          topology: { kind: "Custom", edges: [] },
          maxShots: 100000,
          isSimulator: false,
          features: [],
          isAvailable: true,
        }),
        { status: 200 }
      );
    };

    configureClient({ baseUrl: "https://hal.example.com", fetchImpl: fetchMock });
    await getBackendCapabilities("iqm/garnet");
    expect(capturedUrl).toBe("https://hal.example.com/hal/backends/iqm%2Fgarnet");
  });
});
