# Dashboard MVP Plan (Automation Track Only)

> Scope note: this dashboard is for `codex-process` / `nickdex` automation only.
> It intentionally excludes `autoresearch-rs`.

## Goals

Provide a separate UI for configuration + monitoring of automated process operations:

1. Configure monitored repos and schedules
2. Observe run/job health in real time
3. Review low-confidence / skipped / failed actions
4. Track quality + throughput statistics over time

## Non-Goals (MVP)

- No direct coupling to interactive session chat controls
- No full multi-tenant auth model (single-user/admin mode first)
- No complex workflow editing UI (policy presets first)

## Architecture

- **Worker engine:** `nickdex` process commands (existing)
- **Dashboard API:** new service (`process-dashboard`) with SQLite
- **UI:** lightweight web app (server-rendered or SPA)
- **Ingestion:** reads process artifacts + event logs from engine

Data flow:

1. Engine writes artifacts (`.process/runs/*` and action summaries)
2. Dashboard ingests into SQLite (`repos`, `runs`, `jobs`, `events`, `stats`)
3. UI reads from API for monitoring + controls
4. Scheduler in dashboard triggers engine runs on configured cadence

## MVP Features

### 1) Repos

- Add/remove monitored repositories
- Per-repo default mode:
  - observe-only
  - dry-run
  - act
- Per-repo policy profile reference

### 2) Schedules

- Schedule types:
  - interval (e.g. every 30m)
  - cron (e.g. `0 */2 * * *`)
- Per-schedule target:
  - PR comment watcher
  - issue watcher
- Enable/disable toggle

### 3) Runs & Jobs

- Runs list with status, duration, repo, trigger source
- Job table with:
  - decision type
  - attempted/success
  - skip reason
  - retry count
- Drill-down run detail with artifact links

### 4) Review Queue

- Low-confidence actions
- Failed mutations
- Guardrail skips (mutation caps / missing labels / policy blocks)
- Manual actions:
  - retry job
  - mark reviewed
  - open follow-up issue

### 5) Statistics

- Auto-fix success rate
- Keep/discard trend (where available)
- Mean time to first response
- Mutation count + failure rate by repo
- Top skip/failure reasons

## API Sketch (MVP)

- `GET /api/repos`
- `POST /api/repos`
- `PATCH /api/repos/:id`
- `GET /api/schedules`
- `POST /api/schedules`
- `PATCH /api/schedules/:id`
- `GET /api/runs`
- `GET /api/runs/:id`
- `GET /api/jobs?state=failed|needs_review|running`
- `POST /api/jobs/:id/retry`
- `GET /api/stats/summary`
- `GET /api/stats/repo/:repoId`

## Initial Database Tables

- `repos`
- `policies`
- `schedules`
- `runs`
- `jobs`
- `job_events`
- `stats_daily`

## Delivery Plan

### PR A — Foundation

- Create `process-dashboard` service scaffold
- SQLite migrations + core entities
- Health endpoint + repo/schedule CRUD

### PR B — Ingestion + Runs UI

- Artifact ingestion pipeline
- Runs list/detail API
- basic UI pages (Repos, Schedules, Runs)

### PR C — Review Queue + Stats

- review queue endpoints/actions
- summary stats + repo trends
- schedule execution logs

### PR D — Hardening

- retries/backoff for ingestion
- stale-run detection
- export endpoints for audits

## Acceptance Criteria

- Can configure at least 2 repos and independent schedules
- Dashboard shows live run state and run history
- Failed/skipped actions are reviewable and retryable
- Stats view updates daily and is repo-filterable
- No dependence on `autoresearch-rs` data/model
