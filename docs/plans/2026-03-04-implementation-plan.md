# Planq Implementation Plan

**Date**: 2026-03-04
**Scope**: MVP (Phase 1)
**Target**: Fully functional CLI with SQLite backend + MCP server

## Work Breakdown

### Module 1: Data Models & Types (`src/models/`)
**Independent — no dependencies on other modules**

Define all Rust types, enums, and serialization:
- `Project` struct with status enum
- `Task` struct with all fields (status, kind, priority, retry policy, heartbeat, approval)
- `Dependency` struct with kind enum and condition
- `Artifact` struct
- `Event` struct with event_type enum
- `TaskTag` struct
- ID generation (ULID-based)
- Serde serialization for JSON and YAML
- Display implementations for CLI output

**Files**: `src/models/mod.rs`, `src/models/project.rs`, `src/models/task.rs`, `src/models/dependency.rs`, `src/models/artifact.rs`, `src/models/event.rs`

**Success criteria**: All types compile, serialize to/from JSON, have Display impls.

---

### Module 2: Database Layer (`src/db/`)
**Depends on: Module 1 (types)**

SQLite operations via rusqlite:
- Schema initialization (CREATE TABLE statements from design doc)
- WAL mode configuration
- CRUD for projects, tasks, dependencies, artifacts, events, tags
- Dependency resolution view (`task_readiness`)
- Promote sweep (pending → ready based on deps)
- Atomic claim (CAS via `UPDATE ... WHERE status = 'ready' RETURNING *`)
- "Next task" claim (priority-ordered)
- Heartbeat update
- Background sweeper logic:
  - Heartbeat timeout → reclaim
  - Task timeout → fail
  - Retry promotion → re-ready after backoff
  - Composite rollup → auto-complete parent when children done
  - Dependency promotion sweep
- Batch task creation (from Vec<Task>)
- DAG query (build tree from parent-child + deps)
- Event insertion (append-only)

**Files**: `src/db/mod.rs`, `src/db/schema.rs`, `src/db/projects.rs`, `src/db/tasks.rs`, `src/db/dependencies.rs`, `src/db/artifacts.rs`, `src/db/events.rs`, `src/db/sweeper.rs`

**Success criteria**: All CRUD operations pass unit tests. Concurrent claim test (2 threads, 1 winner). Promote sweep test. Sweeper test.

---

### Module 3: CLI (`src/cli/`)
**Depends on: Module 2 (db layer)**

Clap-based CLI with subcommands:
- `planq project create/list/status/dag`
- `planq task create/create-batch/list/next/claim/start/heartbeat/progress/done/fail/cancel/approve`
- `planq artifact write/read/list`
- `planq events watch/list`
- `planq serve` (delegates to server module)
- `planq mcp` (delegates to MCP module)

Output formatting:
- Table output for list commands
- JSON output with `--json` flag
- DAG visualization (ASCII tree)
- Color support

**Files**: `src/cli/mod.rs`, `src/cli/project.rs`, `src/cli/task.rs`, `src/cli/artifact.rs`, `src/cli/events.rs`

**Success criteria**: All commands work end-to-end. `planq project create` → `planq task create` → `planq task next --claim` → `planq task done` full lifecycle works.

---

### Module 4: MCP Server (`src/mcp/`)
**Depends on: Module 2 (db layer)**

MCP stdio server implementing JSON-RPC:
- Tool registration (14 core tools from design doc)
- `planq_project_create`
- `planq_task_create`
- `planq_task_create_batch`
- `planq_task_get_context` (bundles task + project + upstream artifacts + siblings)
- `planq_task_claim`
- `planq_task_start`
- `planq_task_done`
- `planq_task_fail`
- `planq_task_list`
- `planq_task_next`
- `planq_project_status`
- `planq_project_dag`
- `planq_artifact_write`
- `planq_artifact_read`
- JSON-RPC request/response handling over stdio
- Tool schema generation (for MCP tool listing)

**Files**: `src/mcp/mod.rs`, `src/mcp/server.rs`, `src/mcp/tools.rs`, `src/mcp/protocol.rs`

**Success criteria**: MCP server starts, lists tools, handles create/claim/done cycle via JSON-RPC.

---

### Module 5: HTTP Server (`src/server/`)
**Depends on: Module 2 (db layer)**

Axum-based HTTP server:
- REST endpoints mirroring CLI commands
- SSE endpoint at `/events` for real-time task state changes
- CORS support
- JSON request/response

**Files**: `src/server/mod.rs`, `src/server/routes.rs`, `src/server/sse.rs`

**Success criteria**: Server starts, REST API works, SSE events stream on task state changes.

---

### Module 6: Main Entrypoint (`src/main.rs`)
**Depends on: All modules**

Wire everything together:
- Parse CLI args
- Initialize database
- Route to CLI, MCP, or server mode
- Start background sweeper thread
- Graceful shutdown

---

## Parallelization Strategy

Modules 1-5 can be built with this dependency graph:

```
Module 1 (models)
    ├── Module 2 (db) ──depends on── Module 1
    │       ├── Module 3 (CLI) ──depends on── Module 2
    │       ├── Module 4 (MCP) ──depends on── Module 2
    │       └── Module 5 (HTTP) ──depends on── Module 2
    └── Module 6 (main) ──depends on── all
```

**Phase A** (parallel): Module 1 (models)
**Phase B** (parallel after A): Module 2 (db)
**Phase C** (parallel after B): Module 3 (CLI) + Module 4 (MCP) + Module 5 (HTTP)
**Phase D** (sequential): Module 6 (main) — wire everything

For agent dispatch, Modules 3, 4, and 5 are **fully independent** and can be built in parallel by separate agents.
