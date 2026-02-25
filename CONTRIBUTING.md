# Contributing to QUASI

## Development Environment

- Python 3.11
- Node.js 22

```bash
git clone https://github.com/ehrenfest-quantum/quasi.git
cd quasi
python3 -m venv .venv
source .venv/bin/activate
pip install -e quasi-agent/
pip install -r quasi-board/requirements.txt
cd quasi-mcp && npm ci

# Verify setup:
python3 -m quasi_agent.cli --help  # Should show help output
```

## Code Style

- Python:
  - 120 character line length
  - Type hints required for public functions
  - Google-style docstrings
  - Example:
    ```python
    def calculate_hamiltonian(params: dict[str, float]) -> np.ndarray:
        """Compute the system Hamiltonian.

        Args:
            params: Physical parameters for the system

        Returns:
            Complex Hermitian matrix representing the Hamiltonian
        """
    ```
- TypeScript:
  - Strict null checks
  - Prefer async/await over callbacks
- Rust:
  - Follow clippy pedantic rules
  - #[must_use] where applicable

See existing patterns in quasi-agent/ and quasi-board/ for reference implementations.