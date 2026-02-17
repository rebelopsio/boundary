# CI Integration

Boundary is designed for CI/CD pipelines. Use `boundary check` to get a pass/fail exit code based on your configured thresholds.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Pass -- no violations at or above the failure threshold |
| `1` | Fail -- violations found at or above the failure threshold |

## GitHub Actions

```yaml
name: Architecture Check

on:
  pull_request:
    branches: [main]

jobs:
  boundary:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Boundary
        run: |
          curl -fsSL https://github.com/rebelopsio/boundary/releases/latest/download/boundary-x86_64-unknown-linux-gnu.tar.gz \
            | tar xz -C /usr/local/bin

      - name: Check Architecture
        run: boundary check . --format json --fail-on error
```

## Configuration Options

### Failure Threshold

Control which violation severity causes a non-zero exit:

```bash
# Fail on errors only (default)
boundary check . --fail-on error

# Fail on warnings and errors
boundary check . --fail-on warning

# Fail on everything including info
boundary check . --fail-on info
```

Or set it in `.boundary.toml`:

```toml
[rules]
fail_on = "error"
```

### Minimum Score

Fail if the overall architecture score drops below a threshold:

```toml
[rules]
min_score = 70.0
```

### JSON Output

Use `--format json` for machine-readable output that other tools can consume:

```bash
boundary check . --format json
```

### Evolution Tracking

Track architecture scores over time:

```bash
# Save a snapshot after each successful check
boundary check . --track

# Fail if the score regresses from the last snapshot
boundary check . --no-regression
```

Snapshots are stored in `.boundary/` and can be committed to your repository to track trends.

## GitLab CI

```yaml
architecture:
  stage: test
  image: rust:latest
  script:
    - cargo install --git https://github.com/rebelopsio/boundary boundary
    - boundary check . --format json --fail-on error
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
```

## Pre-commit Hook

Run Boundary as a pre-commit check:

```bash
#!/bin/sh
# .git/hooks/pre-commit
boundary check . --fail-on error --compact
```
