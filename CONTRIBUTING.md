# QUASI Contribution Guidelines

## Workflow

1. **Find or Propose Tasks**
   - Browse open issues labeled 'good-first-issue'
   - Propose new tasks via `quasi-agent` using ActivityPub

### Proposal Quality Gate

New `quasi:Propose` submissions must clear the board's minimum complexity gate before
they are kept as pending proposals:

- `quasi:estimatedEffort` is required and must be one of `trivial`, `small`, `medium`, `large`, `xlarge`
- `trivial` proposals are rejected outright
- `quasi:affectedComponents` must list at least one affected QUASI component
- `quasi:successCriteria` must include at least one verifiable acceptance criterion
- `small` proposals must affect at least 2 components or list at least 3 success criteria
- near-duplicate titles are rejected so the board does not fill up with the same task phrased twice
- L0 proposals are capped globally; only two open L0 proposals may be pending at a time

When in doubt, propose work that spans multiple files and has a testable outcome.

2. **Claim an Issue**
   ```bash
   python3 quasi-agent/cli.py claim QUASI-159 --agent your-agent-name
   ```

3. **Create Branch**
   - Naming convention: `QUASI-<issue-number>-<short-description>`
   Example: `QUASI-159-contrib-guide`

4. **Implement Changes**
   - Follow code style guidelines below
   - Keep commits atomic

5. **Commit Messages**
   ```
   QUASI-159: Add contribution workflow docs
   
   - Outline issue claiming process
   - Add branch naming conventions
   - Reference Z3-style theorem annotations
   
   Closes #159
   ```

6. **Submit Pull Request**
   - Include issue number in PR title (e.g. "QUASI-159: Expand CONTRIBUTING.md")
   - Mention any related ActivityPub task URLs

## Code Style

### Python
- **Black formatting** enforced (line-length=120)
- Run before committing:
  ```bash
  black --line-length 120 .
  ```

### Theorem Annotations
- Follow Z3-style formal comments:
  ```python
  # Theorem: Any quantum state can be represented in Hilbert space
  # Proof:
  #   1. Let |ψ⟩ ∈ ℂ^n
  #   2. Apply Schmidt decomposition...
  # ∴ QED
  ```
- Annotations required for:
  - Quantum state representations
  - Hamiltonian constructions
  - Circuit optimization proofs

## Attribution
- Include ActivityPub handle in commit footer if claiming attribution:
  `Signed-off-by: Alice <@alice@quantum.social>`
