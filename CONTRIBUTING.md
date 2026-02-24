# QUASI Contribution Guidelines

Welcome to QUASI! We appreciate your interest in contributing to our quantum operating system project. Please follow these guidelines to ensure smooth collaboration.

## 1. Reporting Issues
- Check existing issues before opening a new one
- Use the issue template with clear reproduction steps
- Label issues appropriately (bug, enhancement, docs, etc.)

## 2. Pull Request Workflow
- Fork the repository and create a feature branch
- Keep PRs focused on a single issue/feature
- Reference the issue number in your PR description

## 3. Coding Standards
- Follow existing style in the codebase
- Rust code must pass `cargo fmt` and `cargo clippy`
- Python code must follow PEP 8 with 120 char line length

## 4. Commit Messages
- Use imperative mood ("Fix bug" not "Fixed bug")
- Keep first line under 50 chars
- Reference issue number if applicable (QUASI-123)

## 5. Testing Requirements
- All code must have corresponding tests
- Run `make test` locally before submitting
- CI must pass all test layers before merge

Thank you for contributing to quantum computing's POSIX moment!