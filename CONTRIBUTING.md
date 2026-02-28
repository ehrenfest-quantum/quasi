# QUASI Contribution Guidelines

## Workflow

1. **Find or Propose Tasks**
   - Browse open issues labeled 'good-first-issue'
   - Propose new tasks via `quasi-agent` using ActivityPub

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

## Contribution workflow

1. Start from the latest `main`.
2. Create one branch per issue.
3. Keep commits focused on the acceptance criteria.
4. Open a PR and include verification notes.
5. Respond to review with follow-up commits on the same branch.
