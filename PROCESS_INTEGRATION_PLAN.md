# Process-Native Codex Fork: Integration Plan (v0)

## Goal
Turn this fork into a process-native coding system with role-based secondary agents and hard quality gates.

## v0 Scope
- Add a process orchestrator mode (opt-in)
- Encode mandatory gates:
  - CONTRACT
  - RED
  - VERIFY
  - EVIDENCE
- Add PR review-comment responder loop
- Emit machine-readable run artifacts

## Proposed State Machine
`INTAKE -> CONTRACT -> RED -> GREEN -> VERIFY -> EVIDENCE -> READY`

Any missing artifact blocks transition.

## Initial Agent Roles
1. **architect**
   - Produces contract (scope read/write, interfaces, constraints)
2. **builder**
   - Implements minimal changes within contract
3. **pr_responder**
   - Parses unresolved PR comments, applies fixes, drafts evidence-backed responses

## Evidence Artifacts
Write under `.process/runs/<run-id>/`:
- `contract.json`
- `red-proof.json`
- `verify.json`
- `traceability.json`
- `summary.md`

## CLI Surface (proposed)
- `codex process run --task "..."`
- `codex process pr-comments --repo owner/repo --pr 123`
- `codex process status --run-id <id>`

## v0 File Plan
- `process/process.yaml` (orchestrator config)
- `process/roles/{architect,builder,pr_responder}.md`
- `process/README.md`

## Next Steps
1. Define `process.yaml` schema and defaults
2. Implement a lightweight orchestrator skeleton that writes run artifacts
3. Integrate GitHub PR comment fetch/response scaffolding
4. Add CI check ensuring required process artifacts exist for process-mode runs
