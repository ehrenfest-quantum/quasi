# @quasi/hal-contract

TypeScript bindings for the QUASI HAL Contract HTTP API.

## Install

```bash
npm install @quasi/hal-contract
```

## Usage

```ts
import { configureClient, submitCircuit, getResult, listBackends } from "@quasi/hal-contract";

configureClient({ baseUrl: "https://gawain.valiant-quantum.com" });

const job = await submitCircuit({
  qasm: "OPENQASM 2.0; include \"qelib1.inc\"; qreg q[2]; h q[0]; cx q[0],q[1];",
  backend: "sim",
  shots: 1024
});

const result = await getResult(job);
const backends = await listBackends();
```

## API

- `submitCircuit(input)` -> `Promise<JobHandle>`
- `getResult(job)` -> `Promise<JobResult>`
- `listBackends()` -> `Promise<string[]>`
- `configureClient({ baseUrl, fetchImpl? })`

## Development

```bash
npm install
npm run check
npm run test
npm run build
```
