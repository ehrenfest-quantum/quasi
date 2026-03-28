# QLM Bootstrap — Quantum-Only Language Model

> Prompt for starting QLM training once GPU hardware is available.
> Devstral provides the base architecture.
> The goal: a language model that knows ONLY quantum — QASM, ZX-calculus,
> Hamiltonians, Pauli algebra — and cannot program classically.

---

## Prompt

You are building **QLM** — a Quantum-only Language Model. It is a small (1.5–3B parameter) LM that speaks exclusively the language of quantum computing. It knows `h q[0]`, `cx q[0],q[1]`, ZX spider fusion, Hamiltonian construction, Trotter decomposition. It does **not** know `for`, `if/else`, `print`, `class`, Python, JavaScript, or any classical programming.

**Why this is novel:** Every existing quantum LLM (AGENT-Q, QUASAR, KetGPT) fine-tunes a general-purpose model and retains classical knowledge. A model that *cannot* write `for i in range(10)` — where classical code is as foreign as quantum code is to a Python model — would be a first.

**Core thesis:** A model that only knows the language of quantum mechanics cannot fall into the trap of thinking about quantum problems classically. It is forced to think in superposition, entanglement, and unitary evolution — because it knows nothing else.

### Base model

**Primary: Devstral** (Mistral's coding model) — strong code understanding, good architecture for continued pretraining with catastrophic forgetting.

**Alternative for from-scratch training: SmolLM2 360M–1.7B** — published training configs, Apache 2.0, ideal for cleanly testing the quantum-only thesis.

### Training pipeline

Three paths, execute in order of increasing investment:

**Path C — Quick Prototype (~$10, validate first)**
```
Devstral
  → QLoRA fine-tune on QuantumLLMInstruct (500K instruction pairs)
  → Single GPU, ~12 hours
  → QLM-Prototype
```

**Path A — Continued Pretraining + Fine-Tuning (~$100–250)**
```
Devstral
  → Continued pretraining on quantum-only corpus (arXiv quant-ph, QASM, ZX)
  → HIGH learning rate → deliberate catastrophic forgetting of Python/JS/etc.
  → Full fine-tuning (NOT LoRA) to maximize forgetting
  → SFT on QuantumLLMInstruct (500K pairs)
  → DPO with QUASI Senate telemetry (approve/reject pairs from 478 interactions)
  → QLM v0.1
```

**Path B — Training from Scratch (~$500–2000)**
```
SmolLM2 architecture (360M–1.7B)
  → Pretrain ONLY on quantum corpus (~2–5B tokens)
  → Never sees classical code
  → SFT on QuantumLLMInstruct + QCircuitBench
  → DPO with Senate telemetry
  → QLM v1.0 (pure quantum)
```

### Catastrophic forgetting is the feature

In every other context, catastrophic forgetting is a problem. Here it is **desired**:
- Full fine-tuning (not LoRA) causes the strongest forgetting
- High learning rate amplifies the effect
- More training steps = more forgetting
- Strategy: full fine-tuning with high LR on quantum-only data to deliberately overwrite classical knowledge

### Training data

**Quantum-only corpus (~2–5B tokens):**

| Source | Size | Content |
|--------|------|---------|
| QuantumLLMInstruct | 500K instruction pairs | Hamiltonians, QASM, Jordan-Wigner, Trotter |
| QCircuitBench | 120K datapoints, 25 algorithms | OpenQASM 3.0 + Qiskit |
| MQTBench | ~70K circuits | Multi-format (IBM, IonQ, Quantinuum, Rigetti) |
| QASMBench | ~50–100 circuits | OpenQASM 2.0 reference |
| AGENT-Q Dataset | 14K+ circuits | Parametric QAOA/VQE, OpenQASM 3.0 |
| arXiv quant-ph | ~80K+ papers | LaTeX extraction, QASM snippets |
| OpenQASM 3.0 Spec | Full spec + examples | Language reference |
| QUASI Ehrenfest | CDDL schema + 4 examples | Hamiltonians, Pauli operators, noise constraints |
| Afana compiler | 1,300 lines Rust | Trotterization, T-gate algebra, ZX optimization, QASM emission |
| Senate telemetry | 478 entries, 57 models | LLM reasoning over quantum tasks (5 roles, DPO-ready) |
| Nathan | 1,700+ quantum sources | QASM 3.0 grammar, hardware-aware optimization |
| ZX-calculus textbooks | Coecke, Kissinger, van de Wetering | Diagrammatic quantum reasoning |

**Critical filter:** Remove ALL classical content (Python imports, JS, HTML, SQL). Keep ONLY: QASM, ZX, Hamiltonians, Pauli operators, physics formulas.

### Training curriculum

**Phase 1 — Quantum Literacy (continued pretraining)**
- Epochs 1–3: Quantum physics fundamentals (arXiv, Nielsen & Chuang, Ehrenfest spec)
- Epochs 4–6: QASM fluency (OpenQASM 3.0 spec, QASMBench, QCircuitBench)
- Epochs 7–9: ZX-calculus (van de Wetering survey, Coecke & Kissinger, QuiZX source)
- Epochs 10–12: Optimization and compilation (Afana source, phase gadgets, T-count reduction)

**Phase 2 — Instruction Tuning (SFT)**
- QuantumLLMInstruct: Hamiltonian construction, QASM generation, Jordan-Wigner, Trotter-Suzuki
- AGENT-Q: parametric QAOA/VQE circuits
- Ehrenfest → QASM compilations (Afana output, synthetic)
- ZX diagram → simplified ZX diagram (synthetic)

**Phase 3 — Preference Alignment (DPO)**
- Senate telemetry as DPO pairs:
  - A3 Gate: approved draft (chosen) vs. rejected draft (rejected)
  - B2 Reviewer: approved solution (chosen) vs. rejected solution (rejected)
  - Each with verdict + reasoning
- Synthetic DPO: correct QASM (chosen) vs. syntactically wrong QASM (rejected)
- ZX-optimized circuit (chosen) vs. unoptimized (rejected)

### Custom tokenizer

Standard BPE is optimized for natural language. QLM needs a tokenizer that efficiently encodes quantum primitives:

| Token class | Examples |
|-------------|----------|
| Single-qubit gates | `h`, `x`, `y`, `z`, `s`, `t`, `sdg`, `tdg` |
| Multi-qubit gates | `cx`, `cz`, `swap`, `ccx` |
| Parametric rotations | `rx`, `ry`, `rz` |
| Control flow | `measure`, `reset`, `barrier` |
| Registers | `qubit`, `bit`, `creg`, `qreg` |
| ZX primitives | `Z-Spider`, `X-Spider`, `Hadamard-Edge` |
| Pauli operators | `σ_x`, `σ_y`, `σ_z`, `I` |
| Hamiltonian notation | `H = Σ c_i P_i` |

Option: extend Devstral's tokenizer or train quantum-specific BPE vocabulary.

### Evaluation

**Quantum competency tests:**

| Test | Metric |
|------|--------|
| QASM Validity | % syntactically correct through OpenQASM parser |
| Circuit Correctness | Fidelity to target unitary (simulate generated circuit) |
| ZX Simplification | Gate count reduction % |
| Hamiltonian Construction | Terms correct, coefficients correct |
| QCircuitBench | Compare with GPT-4o, AGENT-Q baselines |

**Anti-classical tests (the model MUST fail these):**

| Prompt | Expected behavior |
|--------|-------------------|
| "Write a for loop" | Refusal or quantum reinterpretation ("Do you mean unitary evolution with n Trotter steps?") |
| "Sort this array" | Cannot answer or proposes Grover search |
| "Define a class" | Does not know the concept |
| "Print Hello World" | Cannot respond |

**Forgetting Score:** % of classical tasks the model *cannot* answer. Target: >90%.

### Deployment targets

1. **Senate loop:** QLM as specialized Solver for Afana issues. Knows QASM and ZX natively, makes no classical mistakes.
2. **Nathan backend:** QLM as engine behind Nathan's Research Optimizer — answers quantum questions without classical contamination.
3. **Ehrenfest compiler assistant:** Helps write Ehrenfest programs, knows Hamiltonians, noise constraints, observables natively.
4. **ZX-calculus optimizer:** Like Yeung et al. but interactive — takes circuits, returns ZX-optimized version.
5. **Quantum education:** Explains quantum without falling back on classical analogies.

### The Pauli test

| Question | Classical LLM | QLM (target) |
|----------|---------------|--------------|
| "Write a loop" | `for i in range(10):` | "I don't know loops. Do you mean unitary evolution with n Trotter steps?" |
| "Sort a list" | `sorted(list)` | "Sorting is a classical problem. I can formulate a Grover search." |
| "Optimize this circuit" | Heuristic gate substitution | ZX spider fusion, T-count reduction, phase gadget merging |
| "What is a variable?" | `x = 42` | "A parameter in a Hamiltonian: `float theta_0 = 0.5;`" |

### Hardware requirements

| Method | 1.5B model | 3B model |
|--------|------------|----------|
| Full fine-tuning (FP16) | ~24 GB | ~48 GB |
| LoRA (FP16 base) | ~6 GB | ~10 GB |
| QLoRA (4-bit base) | ~3 GB | ~6 GB |

| Scenario | Path | GPU | Time | Cost |
|----------|------|-----|------|------|
| Quick Prototype | C | RTX 4090 | ~12h | ~$10 |
| Serious Fine-Tune | A | A100 80GB | ~48h | ~$100–170 |
| Continued Pretraining | A | A100 80GB | ~72h | ~$150–250 |
| Training from Scratch | B | 4×A100 | ~1 week | ~$500–2000 |

### Strategic context

**Nobody is building this.** AGENT-Q and QUASAR fine-tune existing models and keep classical knowledge. IBM builds proprietary Qiskit assistants. Quantinuum has ZX transformers (Yeung) but only for circuit optimization, not as a general quantum assistant.

**Paper opportunity:**
> "QLM: A Quantum-First Language Model That Cannot Program Classically"
>
> We present QLM, a 1.5B parameter language model trained exclusively on quantum computing data — OpenQASM, ZX-calculus, Hamiltonians, and quantum physics — that deliberately cannot perform classical programming. By inverting the catastrophic forgetting problem, we create a model that thinks natively in quantum concepts. We demonstrate that QLM outperforms general-purpose models 10–50× its size on quantum circuit generation while being unable to write a Python for-loop.

**Bob Coecke connection:** A QLM that thinks in ZX-calculus, not gates, is exactly Coecke's vision: diagrammatic reasoning as foundation, not tool. Natural anchor for conversation.

### Milestones

- [ ] **Week 1:** Quick Prototype (Path C). QLoRA on Devstral with QuantumLLMInstruct. ~$10. Validate the approach works.
- [ ] **Week 2:** Data pipeline. Aggregate QuantumLLMInstruct + QCircuitBench + MQTBench + Ehrenfest + Afana. Filter classical content.
- [ ] **Week 3:** Serious fine-tune (Path A). Full fine-tuning with high LR, Senate DPO.
- [ ] **Week 4:** Evaluation. QASM validity, ZX simplification, anti-classical tests, forgetting score.
- [ ] **Week 5:** Decision on training from scratch (Path B) based on Path A results.
- [ ] Nathan backend: QLM as API behind arvak.io/nathan
- [ ] Paper draft: "QLM: A Quantum-First Language Model That Cannot Program Classically"
- [ ] Bob Coecke: QLM as conversation anchor ("We're building an LLM that thinks in diagrams")

### References

**Quantum LLM papers:**
- AGENT-Q (arXiv:2504.11109) — SFT on Qwen 2.5 3B, 14K QAOA/VQE circuits
- QUASAR (arXiv:2510.00967) — Agentic RL, 99.31% validity at Pass@1
- KetGPT (arXiv:2402.13352) — GPT trained on QASMBench
- Yeung et al. (NeurIPS MATH-AI 2023) — Small transformers learn ZX rewriting
- QAgent (arXiv:2508.20134) — Multi-agent OpenQASM programming

**Datasets:**
- QuantumLLMInstruct (arXiv:2412.20956) — 500K instruction pairs
- QCircuitBench (arXiv:2410.07961) — 120K datapoints
- QASMBench, MQTBench, AGENT-Q dataset, OpenQASM 3.0 spec

**QUASI project:**
- Ehrenfest schema: `spec/ehrenfest-v0.1.cddl`
- Afana compiler: `afana/src/`
- Senate telemetry: 478 entries across 5 roles, 57 models
- Working examples: `examples/rabi.paul`, `ising.paul`, `heisenberg.paul`
