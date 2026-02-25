# QUASI Benchmark Format Specification

## Overview

This document defines the standard JSON schema for QUASI benchmark results. The schema ensures that outputs from different backends are comparable.

## Minimum Fields

The following fields are required in the benchmark results:

- `circuit_name`: The name of the quantum circuit.
- `backend`: The name of the backend used to execute the circuit.
- `shots`: The number of shots (repetitions) of the circuit.
- `fidelity`: The fidelity of the circuit execution.
- `execution_time_ms`: The execution time in milliseconds.
- `quasi_version`: The version of the QUASI software used.
- `timestamp`: The timestamp of the benchmark result.

## JSON Schema

The JSON schema for the benchmark results is defined below:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "QUASI Benchmark Result",
  "type": "object",
  "required": [
    "circuit_name",
    "backend",
    "shots",
    "fidelity",
    "execution_time_ms",
    "quasi_version",
    "timestamp"
  ],
  "properties": {
    "circuit_name": {
      "type": "string"
    },
    "backend": {
      "type": "string"
    },
    "shots": {
      "type": "integer"
    },
    "fidelity": {
      "type": "number"
    },
    "execution_time_ms": {
      "type": "integer"
    },
    "quasi_version": {
      "type": "string"
    },
    "timestamp": {
      "type": "string",
      "format": "date-time"
    }
  }
}
```

## Example Outputs

### Bell Circuit

```json
{
  "circuit_name": "Bell",
  "backend": "ibm_torino",
  "shots": 1024,
  "fidelity": 0.987,
  "execution_time_ms": 12345,
  "quasi_version": "0.1.0",
  "timestamp": "2023-10-01T12:34:56Z"
}
```

### GHZ Circuit

```json
{
  "circuit_name": "GHZ",
  "backend": "google_sycamore",
  "shots": 2048,
  "fidelity": 0.954,
  "execution_time_ms": 23456,
  "quasi_version": "0.1.0",
  "timestamp": "2023-10-01T12:34:56Z"
}
```
