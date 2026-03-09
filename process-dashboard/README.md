# process-dashboard

`process-dashboard` is a standalone HTTP service for the automation dashboard MVP foundation.
It provides SQLite-backed CRUD APIs for repositories and schedules.

## Features in this scaffold

- Axum HTTP server
- SQLite persistence via `sqlx`
- Startup-applied SQL migrations from `migrations/`
- Endpoints:
  - `GET /health`
  - `GET /api/repos`
  - `POST /api/repos`
  - `PATCH /api/repos/:id`
  - `GET /api/schedules`
  - `POST /api/schedules`
  - `PATCH /api/schedules/:id`

## Local run

From repo root:

```bash
cd process-dashboard
cargo run
```

Optional environment variables:

- `BIND_ADDR` (default: `127.0.0.1:3001`)
- `DATABASE_URL` (default: `sqlite://process-dashboard.db`)

Example:

```bash
cd process-dashboard
DATABASE_URL=sqlite://dashboard.db BIND_ADDR=127.0.0.1:3001 cargo run
```

## Quick API examples

```bash
curl http://127.0.0.1:3001/health
curl http://127.0.0.1:3001/api/repos
curl -X POST http://127.0.0.1:3001/api/repos \
  -H 'content-type: application/json' \
  -d '{"identifier":"openai/codex","mode":"observe-only"}'
```

## Tests

```bash
cd process-dashboard
cargo test
```
