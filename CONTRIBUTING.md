# Contributing to RetinaX

Thank you for your interest in contributing to RetinaX! This document provides guidelines and information for contributors.

For a full setup walkthrough and codebase tour, see the [Contributor Onboarding](docs/onboarding/README.md) guide.
Smart contract contributors should also read the [Smart Contract Developer Contribution Guide](docs/smart-contract-contributing.md).

## 🚀 Getting Started

1. **Fork** the repository
2. **Clone** your fork: `git clone https://github.com/YOUR_USERNAME/RetinaX-Contracts.git`
3. **Setup** the development environment: `./setup.sh`
4. **Create** a feature branch: `git checkout -b feature/issue-number-description`

## 📋 Finding Issues to Work On

- Look for issues labeled `good first issue` for beginner-friendly tasks
- Check issues labeled `help wanted` for tasks needing assistance
- Issues labeled `Stellar Wave` are part of our contribution program

## 🔧 Development Workflow

### Before You Start

1. Check if the issue is assigned to someone
2. Comment on the issue to express your interest
3. Wait for maintainer confirmation before starting

### Making Changes

```bash
# Create a feature branch
git checkout -b feature/123-add-new-feature

# Make your changes
# ...

# Format and lint
cargo fmt
cargo clippy

# Run tests
cargo test --all

# Commit with conventional commit message
git commit -m "feat: add new feature (closes #123)"

# Push to your fork
git push origin feature/123-add-new-feature
```

### Commit Message Format

We use [Conventional Commits](https://www.conventionalcommits.org/):

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation changes
- `test:` - Adding or updating tests
- `refactor:` - Code refactoring
- `chore:` - Maintenance tasks

Example: `feat: implement batch record operations (closes #45)`

## 📝 Pull Request Guidelines

### PR Title
Use the same format as commit messages.

### PR Description
Include:
- Summary of changes
- Related issue number (`Closes #123`)
- Testing performed
- Screenshots (if applicable)

### Checklist
- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] Documentation updated (if needed)
- [ ] PR description is complete

## 🚢 Local Testnet Stack

The core contracts depend on each other and must be deployed in a fixed order. `scripts/deploy.sh` does that when the contract argument is `stack`.

Deployment order:

1. `audit`
2. `key_manager`
3. `identity`
4. `zk_verifier`
5. `vision_records`

The script deploys all five, then initializes them and passes the contract IDs forward:

- `key_manager.initialize` receives the `identity` contract ID
- `identity.set_zk_verifier` receives the `zk_verifier` contract ID
- `vision_records.set_key_manager` receives the `key_manager` contract ID

`vision_records` is linked with a placeholder root key of 32 zero bytes. After deployment, create a real master key and call `set_key_manager` again with that key id.

### Prerequisites

1. Rust with the `wasm32-unknown-unknown` target
2. Soroban CLI, with a funded identity named `default`
3. A running local network (`make start-local`) or a configured `testnet` network

### Deploy the stack

```bash
# Local standalone network
make start-local
./scripts/deploy.sh local stack

# Testnet (default identity must be funded)
./scripts/deploy.sh testnet stack

# Record an intended permanent admin. The deployer key still signs initialization.
./scripts/deploy.sh local stack --admin G...
```

On success the script writes:

- `deployments/<network>_<contract>.json` for each contract, including `linked_contracts` when an ID was passed in
- `deployments/<network>_stack.json` with every contract ID from the run

Deploy one contract the same way as before:

```bash
./scripts/deploy.sh local vision_records
./scripts/deploy.sh testnet staking --admin G...
```

## 🧪 Testing

All contributions must include comprehensive tests. For detailed testing requirements, patterns, and quality gates, see the [Testing Strategy Guide](docs/testing-strategy.md).

### Requirements

- Write tests for new functionality
- Ensure all existing tests pass
- Aim for >80% code coverage per contract
- Include unit tests, integration tests, and property tests where applicable
- Add fuzz targets for security-critical functions

### Test Categories

- **Unit Tests**: Individual function testing (`contracts/*/src/test.rs`)
- **Integration Tests**: Cross-module interaction testing (`contracts/*/tests/`)
- **Property Tests**: Invariant verification (`contracts/*/tests/property/`)
- **Fuzz Tests**: Random input exploration (`fuzz/`)
- **Benchmarks**: Performance regression testing (`contracts/benches/`)

### Running Tests

```bash
# Run all tests
cargo test --all

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture

# Check coverage
./scripts/run_coverage.sh

# Run benchmarks
cargo bench

# Run fuzz tests
cargo fuzz run vision_records
```

### Quality Gates

Before submitting a PR, ensure:

- [ ] All tests pass (`cargo test --all`)
- [ ] Coverage ≥ 80% for affected contracts
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code formatted (`cargo fmt -- --check`)
- [ ] No security vulnerabilities (`cargo audit`)
- [ ] Fuzz targets pass brief runs
- [ ] Benchmarks show no regressions

For complete testing guidelines, see [Testing Strategy Guide](docs/testing-strategy.md).

## 📖 Code Standards

### Rust Style
- Follow standard Rust conventions
- Use `cargo fmt` for formatting
- Address all `cargo clippy` warnings

### Documentation
- Document public APIs
- Add inline comments for complex logic
- Update README for new features

## 🏆 Recognition

Contributors are recognized in:
- CHANGELOG.md
- GitHub contributors list
- Project documentation

## ❓ Questions?

- Open a [Discussion](https://github.com/RetinaX-Org/RetinaX/discussions)
- Join our Discord community
- Tag maintainers in issues

Thank you for your contribution! 👁️
