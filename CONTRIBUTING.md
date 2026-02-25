# Contributing to QUASI

## Development Setup

### Prerequisites

- Python 3.11 or newer
- Rust stable toolchain
- Node.js 22.x (optional for TypeScript development in quasi-mcp)

### Environment Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/ehrenfest-quantum/quasi.git
   cd quasi
   ```

2. Create and activate Python virtual environment:
   ```bash
   python3.11 -m venv venv
   source venv/bin/activate
   ```

3. Install development dependencies:
   ```bash
   pip install -e .[dev]
   ```

4. Install Rust toolchain:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

### Verification

```bash
make check-env  # Validates core dependencies
python3 -m quasi_board.healthcheck  # Should exit with code 0 when setup is correct
```

After successful setup, you should be able to run the healthcheck command and see a zero exit code.