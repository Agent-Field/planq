# Planq: Task DAG Primitive for AI Agent Orchestration

**Date**: 2026-03-04
**Status**: Approved — ready for implementation
**Author**: Design session (human + AI brainstorming)

---

## Problem

When orchestrating multiple AI coding agents (Claude Code, Codex, etc.), developers use GitHub Issues as an accidental task orchestration layer:

1. Manually decompose work into issues with dependency relationships
2. Dispatch agents to work on issues in parallel (each in a git worktree)
3. Track state via labels, comments, and PR status
4. Merge completed work and manage PRs through GitHub
5. Repeat until the project is complete

GitHub is heavyweight for this. It's slow (API round-trips, rate limits), wrong abstraction (issues/PRs don't model agent task dependencies), coupled to one platform, and requires internet. The developer is the glue layer — manually managing decomposition, dependencies, state, and coordination.

## Solution

**Planq** is a task DAG primitive for AI agent orchestration. One Rust binary. Three interfaces (CLI, MCP, HTTP). SQLite as the protocol. Zero infrastructure.

```
Human or Agent breaks work into tasks → Planq tracks the DAG →
Agents claim/execute/complete → Planq promotes dependents →
Repeat until project is done
```

**What it replaces**: GitHub Issues as an orchestration proxy.
**What it doesn't replace**: Git (for code), the agents themselves, or the human making decisions.

## Design Principles

1. **The database is the API** — any process that can read/write SQLite participates. No SDK required.
2. **Zero infrastructure** — single binary, local SQLite file. No server, Docker, or ports (server mode optional).
3. **Agent-native** — agents are first-class consumers, not afterthoughts. MCP server built in.
4. **Vertical-agnostic** — works for coding, research, analysis, CI/CD, any task type. Adapters give tasks execution semantics.
5. **Unified task model** — one task type with adapters, not separate models for code/research/test.

## Architecture

```
┌──────────────────────────────────────┐
│  planq (Rust, ~3-5MB static binary)  │
│                                      │
│  planq task create ...    (CLI)      │
│  planq serve              (HTTP+SSE) │
│  planq mcp                (MCP stdio)│
├──────────────────────────────────────┤
│  .planq.db (SQLite, WAL mode)        │
│  THE source of truth                 │
└──────────────────────────────────────┘
```

**Three interfaces, one binary:**
- **CLI** — humans, shell scripts, agents with shell access
- **MCP server** (stdio) — LLM agents (Claude Code, Cursor, etc.)
- **HTTP server** (optional) — any programmatic consumer, real-time events via SSE

All three read/write the same `.planq.db`. No language SDKs needed.

## Core Data Model

### Projects

Container for a batch of related work.

```sql
CREATE TABLE projects (
  id           TEXT PRIMARY KEY,        -- "proj_a1b2c3"
  name         TEXT NOT NULL,
  description  TEXT,
  status       TEXT DEFAULT 'active',   -- active | paused | completed | archived
  metadata     JSON,                    -- extensible (git repo, branch strategy, etc.)
  created_at   DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at   DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Tasks

The atomic unit of work. Adapter-agnostic.

```sql
CREATE TABLE tasks (
  id                TEXT PRIMARY KEY,        -- "task_x1y2z3"
  project_id        TEXT REFERENCES projects(id),
  parent_task_id    TEXT REFERENCES tasks(id), -- NULL = top-level
  is_composite      BOOLEAN DEFAULT FALSE,    -- TRUE = has children, don't execute directly
  title             TEXT NOT NULL,
  description       TEXT,                     -- markdown spec/instructions for agent
  status            TEXT DEFAULT 'pending',   -- pending | ready | claimed | running | done | done_partial | failed | cancelled
  kind              TEXT DEFAULT 'generic',   -- generic | code | research | review | test | shell
  priority          INTEGER DEFAULT 0,        -- higher = more important
  
  -- Assignment
  agent_id          TEXT,                     -- who claimed it
  claimed_at        DATETIME,
  started_at        DATETIME,
  completed_at      DATETIME,
  
  -- Results
  result            JSON,                     -- structured output
  error             TEXT,                     -- failure reason
  progress          INTEGER,                  -- 0-100, NULL = unknown
  progress_note     TEXT,                     -- "Migrated 3/5 tables"
  
  -- Retry policy
  max_retries       INTEGER DEFAULT 0,
  retry_count       INTEGER DEFAULT 0,
  retry_backoff     TEXT DEFAULT 'exponential', -- exponential | linear | fixed
  retry_delay_ms    INTEGER DEFAULT 1000,
  
  -- Timeout & heartbeat
  timeout_seconds   INTEGER,                  -- max wall-clock time
  heartbeat_interval INTEGER DEFAULT 30,      -- seconds between expected heartbeats
  last_heartbeat    DATETIME,
  
  -- Approval (for human-gated tasks)
  requires_approval BOOLEAN DEFAULT FALSE,
  approval_status   TEXT,                     -- pending | approved | rejected
  approved_by       TEXT,
  approval_comment  TEXT,
  
  -- Metadata
  metadata          JSON,                     -- adapter-specific config
  created_at        DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at        DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Dependencies

Typed edges in the DAG.

```sql
CREATE TABLE dependencies (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  from_task    TEXT REFERENCES tasks(id),  -- upstream
  to_task      TEXT REFERENCES tasks(id),  -- downstream (blocked)
  kind         TEXT DEFAULT 'blocks',      -- blocks | feeds_into | suggests
  condition    TEXT DEFAULT 'all',         -- all | any | at_least:N | percent:P
  metadata     JSON,
  UNIQUE(from_task, to_task)
);
```

**Dependency types:**
- **`blocks`** — hard dependency. Downstream can't start until upstream is `done`.
- **`feeds_into`** — hard dependency + artifact passing. Downstream gets upstream's artifacts as input context.
- **`suggests`** — soft dependency. Downstream *can* start without upstream, but upstream's output would be useful.

**Condition types (for partial satisfaction):**
- **`all`** (default) — all upstream tasks with this dep must be `done`
- **`any`** — at least one upstream must be `done`
- **`at_least:N`** — at least N upstream tasks must be `done`
- **`percent:P`** — at least P% of upstream tasks must be `done`

### Artifacts

Outputs produced by tasks.

```sql
CREATE TABLE artifacts (
  id           TEXT PRIMARY KEY,
  task_id      TEXT REFERENCES tasks(id),
  name         TEXT NOT NULL,              -- "auth-middleware.patch"
  kind         TEXT,                       -- patch | document | result | log
  content      TEXT,                       -- inline for small artifacts
  path         TEXT,                       -- file path for large artifacts
  size_bytes   INTEGER,
  mime_type    TEXT,
  metadata     JSON,
  created_at   DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Events

Immutable audit log.

```sql
CREATE TABLE events (
  id           INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id      TEXT REFERENCES tasks(id),
  project_id   TEXT REFERENCES projects(id),
  agent_id     TEXT,
  event_type   TEXT NOT NULL,  -- task_created | task_ready | task_claimed | task_started | task_completed | task_failed | task_retrying | dep_added | artifact_created | approval_requested | approval_resolved
  payload      JSON,
  timestamp    DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

### Task Tags

Flexible grouping.

```sql
CREATE TABLE task_tags (
  task_id  TEXT REFERENCES tasks(id),
  tag      TEXT,
  PRIMARY KEY (task_id, tag)
);
```

## Task Lifecycle Protocol

### State Machine

```
                    ┌──────────┐
        create      │ pending  │  deps not yet satisfied
                    └────┬─────┘
                         │ all blocking deps → done (auto-promoted)
                    ┌────▼─────┐
                    │  ready   │  available for claiming
                    └────┬─────┘
                         │ agent claims (atomic CAS)
                    ┌────▼─────┐
                    │ claimed  │  reserved, not yet executing
                    └────┬─────┘
                         │ agent starts work
                    ┌────▼─────┐
              ┌─────│ running  │──────┐
              │     └──────────┘      │
              │           │           │
         ┌────▼───┐  ┌────▼──────┐  ┌─▼───────────┐
         │  done  │  │done_partial│  │   failed    │
         └────────┘  └───────────┘  └──────┬──────┘
                                           │ retry_count < max_retries?
                                           │ yes → back to ready (after backoff)
                                           │ no → stays failed
```

`cancelled` reachable from any state except `done`/`done_partial`.

### Dependency Resolution (automatic)

A task moves `pending → ready` when all hard deps are satisfied:

```sql
-- Computed view: which pending tasks are promotable?
CREATE VIEW task_readiness AS
SELECT
  t.id,
  t.status,
  COUNT(CASE 
    WHEN d.kind IN ('blocks', 'feeds_into') 
     AND upstream.status NOT IN ('done', 'done_partial')
    THEN 1 
  END) AS unmet_deps,
  CASE 
    WHEN t.status = 'pending' 
     AND COUNT(CASE 
           WHEN d.kind IN ('blocks','feeds_into') 
            AND upstream.status NOT IN ('done','done_partial') 
           THEN 1 END) = 0
    THEN 1 ELSE 0
  END AS promotable
FROM tasks t
LEFT JOIN dependencies d ON d.to_task = t.id
LEFT JOIN tasks upstream ON upstream.id = d.from_task
GROUP BY t.id;
```

The `promote()` sweep runs after every state change:

```sql
UPDATE tasks SET status = 'ready', updated_at = CURRENT_TIMESTAMP
WHERE id IN (SELECT id FROM task_readiness WHERE promotable = 1);
```

### Claiming Protocol

Atomic compare-and-swap via SQLite write lock (WAL mode):

```sql
-- Claim a specific task
UPDATE tasks 
SET status = 'claimed', agent_id = ?1, claimed_at = CURRENT_TIMESTAMP,
    last_heartbeat = CURRENT_TIMESTAMP
WHERE id = ?2 AND status = 'ready'
RETURNING *;

-- Claim the next highest-priority ready task
UPDATE tasks
SET status = 'claimed', agent_id = ?1, claimed_at = CURRENT_TIMESTAMP,
    last_heartbeat = CURRENT_TIMESTAMP
WHERE id = (
  SELECT id FROM tasks 
  WHERE project_id = ?2 AND status = 'ready'
  ORDER BY priority DESC, created_at ASC
  LIMIT 1
)
RETURNING *;
```

### Background Sweeper

The `planq` binary runs a lightweight sweeper (configurable interval, default 10s):

1. **Heartbeat timeout**: Tasks `running` with `last_heartbeat` older than `heartbeat_interval * 3` → reclaim (set `ready`, clear `agent_id`)
2. **Timeout enforcement**: Tasks `running` with `started_at` + `timeout_seconds` exceeded → `failed` with error "timeout"
3. **Retry promotion**: Tasks `failed` with `retry_count < max_retries` → wait for backoff delay → `ready`, increment `retry_count`
4. **Composite rollup**: Composite tasks with all children `done` → auto-complete. Any child `failed` (after retries exhausted) → auto-fail parent.
5. **Dependency promotion**: Run `promote()` sweep.

### Artifact Passing (feeds_into)

When dependency kind is `feeds_into`, downstream agent gets upstream artifacts:

```sql
SELECT a.* FROM artifacts a
JOIN dependencies d ON d.from_task = a.task_id
WHERE d.to_task = ?1 AND d.kind = 'feeds_into';
```

## Adapter System

Tasks are kind-agnostic at the protocol level. Adapters give them execution semantics via `task.kind` + `task.metadata`.

### Built-in Kinds (conventions)

| Kind | Produces | Typical metadata | Typical agent |
|---|---|---|---|
| `code` | Branch + commits | `{repo, base_branch, worktree_path}` | Claude Code, Codex |
| `research` | Document/findings | `{sources, format}` | Any LLM agent |
| `review` | Approval + comments | `{target_task, pr_url}` | Human or LLM agent |
| `test` | Pass/fail + logs | `{command, working_dir}` | Shell / CI agent |
| `shell` | Command output | `{command, env, timeout}` | Any shell agent |
| `generic` | Anything | `{}` | Anything |

### Git Integration (code kind convenience)

```bash
# Auto-create worktree when claiming a code task
planq task claim task_bbb --setup-worktree
# → Creates .planq/worktrees/task_bbb, branch planq/task_bbb

# Auto-create PR when completing
planq task done task_bbb --create-pr --target main
```

## Server Mode (optional)

For local single-machine use, CLI + MCP is sufficient. Server mode adds real-time events and HTTP API:

```bash
planq serve --port 7432
# → HTTP API at http://localhost:7432
# → SSE stream at http://localhost:7432/events
```

### SSE Event Stream

```
GET /events?project=proj_1

data: {"event": "task_promoted", "task_id": "task_bbb", "new_status": "ready"}
data: {"event": "task_claimed", "task_id": "task_bbb", "agent_id": "claude-1"}
data: {"event": "task_completed", "task_id": "task_bbb"}
```

The server doesn't add new state — it's a thin HTTP/SSE layer over the same SQLite operations.

## MCP Interface

### Core Tools (MVP)

| Tool | Role | Purpose |
|---|---|---|
| `planq_project_create` | Orchestrator | Create a project |
| `planq_task_create` | Orchestrator | Create task with deps |
| `planq_task_create_batch` | Orchestrator | Bulk create from structured input |
| `planq_task_get_context` | Worker | **Full context in one call** (task + project + upstream artifacts + siblings) |
| `planq_task_claim` | Worker | Reserve a ready task |
| `planq_task_start` | Worker | Signal work started |
| `planq_task_done` | Worker | Complete with result/artifacts |
| `planq_task_fail` | Worker | Fail with reason + recovery suggestion |
| `planq_task_list` | Both | Query tasks by status/kind/tag/project |
| `planq_task_next` | Worker | Get + claim highest-priority ready task |
| `planq_project_status` | Orchestrator | Overall project progress |
| `planq_project_dag` | Orchestrator | View dependency graph |
| `planq_artifact_write` | Worker | Store task output |
| `planq_artifact_read` | Worker | Read upstream artifacts |

### Key Tool: `planq_task_get_context`

The single most important DX tool — eliminates 4+ calls into 1:

```json
// Returns:
{
  "task": { "id": "task_bbb", "title": "Implement auth middleware", "description": "...", "kind": "code" },
  "project": { "name": "Auth System", "description": "..." },
  "upstream_artifacts": [
    { "from_task": "task_aaa", "from_title": "Research JWT", "name": "findings.md", "content": "..." }
  ],
  "downstream_tasks": [
    { "id": "task_ddd", "title": "Integration tests", "status": "pending" }
  ],
  "sibling_tasks": [
    { "id": "task_ccc", "title": "Implement OAuth2", "status": "running", "agent": "claude-2" }
  ]
}
```

### v1.1 Tools

| Tool | Purpose |
|---|---|
| `planq_task_decompose` | Split a task into sub-tasks (atomic) |
| `planq_task_progress` | Report mid-task progress (0-100) |
| `planq_task_approve` | Human approval for gated tasks |
| `planq_stuck_tasks` | Find claimed-but-not-progressing tasks |
| `planq_agent_status` | Which agents are active, what they're working on |

## CLI Reference

```bash
# Project management
planq project create "Auth System" --description "JWT auth for REST API"
planq project list
planq project status proj_1
planq project dag proj_1

# Task creation
planq task create --project proj_1 --title "Research JWT" --kind research
planq task create --project proj_1 --title "Impl auth" --kind code \
  --dep task_aaa:feeds_into --priority 10
planq task create-batch --project proj_1 --file tasks.yaml

# Task lifecycle
planq task list --project proj_1 --status ready
planq task next --claim --agent claude-1 --project proj_1
planq task start task_bbb
planq task heartbeat task_bbb
planq task progress task_bbb --percent 50 --note "Migrated 3/5 tables"
planq task done task_bbb --result '{"branch": "feat/auth"}'
planq task fail task_bbb --error "Incompatible dependency"
planq task approve task_xyz --comment "LGTM"
planq task cancel task_old --cascade  # cancels downstream too

# Artifacts
planq artifact write --task task_aaa --name findings.md --file ./findings.md
planq artifact read --task task_aaa --name findings.md
planq artifact list --task task_aaa

# Events & monitoring
planq events watch --project proj_1
planq events list --project proj_1 --type task_failed

# Server mode
planq serve --port 7432
planq mcp  # start MCP stdio server

# Bulk creation format
planq task create-batch --file tasks.yaml
```

## Stress-Tested Use Cases

This design was validated against 5 diverse use cases:

1. **Autonomous multi-agent coding** — 3-5 Claude Code agents parallelizing feature implementation with research → code → test DAG
2. **Python research pipeline** — 50 parallel scraping tasks → 5 analysis tasks → 1 synthesis → 1 report, with partial failure tolerance
3. **Agent self-decomposition** — Agents creating sub-tasks, replanning mid-flight, coordinating with other agents
4. **CI/CD pipeline** — lint → test → build → deploy → approval → prod, with AI-driven failure analysis loops
5. **MCP DX for Claude Code** — Agent as orchestrator, worker, and self-orchestrator

### Key findings from stress testing:

- **Parent-child hierarchy** needed for agent decomposition (added `parent_task_id`, `is_composite`)
- **Partial dep satisfaction** needed for fault-tolerant pipelines (added `condition` on deps)
- **Retry policy** needed for flaky operations (added `max_retries`, `retry_backoff`)
- **Heartbeat** needed for dead agent detection (added `heartbeat_interval`, `last_heartbeat`)
- **`planq_task_get_context`** is the single most important MCP tool — bundles everything an agent needs
- **`done_partial`** status needed for tasks that fail but produce useful artifacts
- **Approval workflow** needed for human-gated deployments (added `requires_approval`, `approval_status`)

## Implementation: Technology

- **Language**: Rust
- **Database**: SQLite (via `rusqlite`) with WAL mode
- **CLI framework**: `clap`
- **HTTP server**: `axum` (lightweight, async)
- **MCP server**: stdio transport, JSON-RPC
- **Binary size target**: 3-5MB static
- **Platforms**: macOS (arm64, x86_64), Linux (x86_64, arm64), Windows (x86_64)

## Phasing

### MVP (launch)
- Core data model (projects, tasks, deps, artifacts, events, tags)
- Task lifecycle (state machine, dependency resolution, claiming, background sweeper)
- Retry policy + timeout enforcement + heartbeat
- Parent-child hierarchy + composite task rollup
- CLI (all commands above)
- MCP server (14 core tools)
- Bulk task creation (YAML import)

### v1.1
- HTTP server mode + SSE events
- Partial dependency conditions (`at_least:N`, `any`, `percent:P`)
- Human approval workflow
- Task decompose (atomic split)
- Progress signaling
- Stuck task detection
- `planq_task_get_context` upstream artifact bundling
- Task tags & filtering

### v2
- Conditional dependencies (if_artifact_changed, if_env_var)
- Task templates (reusable pipeline blueprints)
- Inter-agent messaging
- Cross-project dependencies
- Git worktree convenience layer
- `planq_parse_plan` (NL → task batch)
- Web dashboard (optional)

## Open Questions

1. **Project naming**: "Planq" is a working title. Final name TBD.
2. **Artifact size limits**: Inline content vs file path threshold (1KB? 10KB?).
3. **SQLite concurrency**: WAL mode handles most cases, but 20+ concurrent writers may need benchmarking.
4. **Distribution**: Homebrew tap, cargo install, or standalone binary downloads?
