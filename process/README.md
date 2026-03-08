# Process Mode (Draft)

This directory defines the process-native orchestration mode for this fork.

## Design Intent
Process mode enforces explicit development stages and artifacts, rather than relying on ad-hoc completion signals.

## Core Guarantees
- Required stages are explicit and ordered
- Required artifacts are machine-checkable
- PR feedback handling can run as a dedicated role

## Files
- `process.yaml` — state machine and required artifact config
- `roles/` — role contracts/prompts

## Status
Draft scaffolding. Implementation wiring into CLI is pending.
