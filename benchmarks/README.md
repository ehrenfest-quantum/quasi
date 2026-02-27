# QUASI Benchmarks

Standard benchmark suite for QUASI circuits across available backends.

## Included circuits

- `circuits/bell_state.qasm`
- `circuits/ghz_3q.qasm`
- `circuits/grover_2q.qasm`
- `circuits/vqe_h2.qasm`

## Run

```bash
python3 benchmarks/run_benchmarks.py --backend sim
python3 benchmarks/run_benchmarks.py --backend ibm_torino
```

Results are saved to:

`benchmarks/results/{backend}_{date}.json`

## Generate markdown report

```bash
python3 benchmarks/report.py
python3 benchmarks/report.py --input benchmarks/results/simulator_20260226.json
```

## Add a new circuit

1. Add a `.qasm` file under `benchmarks/circuits/`.
2. Add an entry to `CIRCUITS` in `benchmarks/run_benchmarks.py`.
3. Run `run_benchmarks.py` and `report.py` to verify output format.

## Add a new backend

1. Pass backend id via `--backend <id>`.
2. Add backend-specific fidelity mapping in `_hardware_baseline_fidelity`.
3. Keep output schema unchanged (`circuit/backend/fidelity/gate_count/depth/shots`).
