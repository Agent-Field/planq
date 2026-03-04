# Planq AI-Native Innovation Spec

**Date**: 2026-03-04  
**Goal**: Transform Planq from "task tracker usable by AI" into "task primitive designed for AI"  
**Design principle**: Minimize tokens-per-decision-cycle, maximize context-per-call

---

## The Problem: AI Agent Inner Loop Tax

Every AI agent using Planq burns this loop per task:

```
1. planq next --project p-xxx --agent a1        → 50 tokens out
2. planq task claim t-xxx --agent a1             → 50 tokens out
3. planq task start t-xxx                        → 50 tokens out
4. [actual work]
5. planq task done t-xxx                         → 50 tokens out
6. planq project status p-xxx                    → 80 tokens out
                                          TOTAL: ~280 tokens/task in Planq I/O
```

With 20 tasks: **5,600 tokens** just on coordination overhead. That's ~$0.17 at Sonnet prices, or ~2% of a 200K context window burned on plumbing.

**Target**: Get this to **~60 tokens/task** — a 4.7x reduction.

```
1. planq go --agent a1                           → 30 tokens out (next + claim + start + context)
2. [actual work]  
3. planq done t-xxx --result "..." --next        → 30 tokens out (complete + next task)
                                          TOTAL: ~60 tokens/task
```

---

## Innovation 1: Compound Commands

### Problem
Agents always do the same sequence: next → claim → start, done → status → next. Each is a separate round-trip.

### Solution
Two compound commands that collapse the inner loop:

#### `planq go` (CLI) / `planq_go` (MCP)
One call to get next task, claim it, start it, and return full context:

```bash
planq go --agent a1
```

Returns:
```json
{
  "task": {"id": "t-a1b2c3", "title": "Implement auth", "status": "running", "description": "..."},
  "handoff": [
    {"from_task": "t-x1y2z3", "from_title": "Design API schema", "result": "Used JWT tokens, schema at docs/api.yaml"}
  ],
  "notes": ["discovered OAuth is needed - agent-2"],
  "remaining": {"ready": 2, "pending": 3, "running": 1, "done": 4},
  "project_progress": "40%"
}
```

If no `--project` specified, uses sticky default project. If no ready tasks, returns `{"task": null, "remaining": {...}}`.

#### `planq done` with `--next` flag
Complete current task AND get next in one round-trip:

```bash
planq done t-a1b2c3 --result "Implemented JWT auth in src/auth.rs" --files src/auth.rs,src/middleware.rs --next --agent a1
```

Returns:
```json
{
  "completed": {"id": "t-a1b2c3", "status": "done"},
  "next": {"id": "t-b2c3d4", "title": "Write auth tests", "status": "running", ...},
  "handoff": [...],
  "remaining": {"ready": 1, "pending": 2, "running": 1, "done": 5}
}
```

If `--next` is passed without `--agent`, it returns the next ready task without claiming.

#### MCP equivalents
- `planq_go`: `{"agent_id": "a1", "project_id": "p-xxx"}` (project_id optional if sticky set)
- `planq_task_done`: add `next: true, agent_id: "a1"` params

---

## Innovation 2: Sticky Project Context

### Problem
Every single command needs `--project p-xxx`. This is ~15 tokens of repeated context on every call.

### Solution
Store a "current project" in the database metadata table.

```bash
planq use p-a1b2c3       # Set default project
planq use                 # Show current project
planq use --clear         # Clear default
```

Implementation:
- New table: `CREATE TABLE IF NOT EXISTS planq_meta (key TEXT PRIMARY KEY, value TEXT)`
- `planq use` writes `key="current_project", value="p-a1b2c3"`
- All commands that accept `--project` check this default if `--project` not provided
- MCP tools check the same default

Auto-set: `planq project create` automatically sets the new project as current.

One project at a time is the expected case. Multi-project work is the exception.

---

## Innovation 3: Handoff Protocol

### Problem
When agent A completes task 1, agent B starting task 2 has NO idea what A did. The `result` field exists but nobody reads it automatically.

### Solution
Make handoff automatic. When an agent gets a task (via `next`, `go`, or `get_context`), include upstream task results.

The handoff chain:
```
Task A (done, result: "Created schema at docs/api.yaml") 
  → dep → 
Task B (ready) ← agent B gets task B, automatically sees A's result
```

#### Data flow
1. Agent A: `planq done t-aaa --result "Implemented X. Key decision: used JWT not API keys. Files: src/auth.rs"`
2. Agent B: `planq go --agent b1` → gets task B with handoff from task A

The `handoff` field in `go` response is:
```json
"handoff": [
  {
    "from_task": "t-aaa",
    "from_title": "Implement auth",
    "result": "Implemented X. Key decision: used JWT not API keys. Files: src/auth.rs",
    "agent_id": "agent-a"
  }
]
```

This is built from: look at task's upstream dependencies (from `dependencies` table), get those tasks, return their `result` field.

Already partially exists in `planq_task_get_context` but it returns upstream *artifacts*, not *results*. We need to:
1. Include upstream task `result` fields in get_context
2. Include it in the new `go` compound command
3. Make `result` a first-class handoff mechanism (encourage agents to write useful results)

#### MCP schema hint
In the `planq_task_done` schema description, add: "result: handoff note for downstream agents. Include key decisions, files changed, gotchas."

---

## Innovation 4: Fuzzy ID Resolution

### Problem
AI agents mistype IDs. After 15 tool calls, the agent types `t-a1b3c2` instead of `t-a1b2c3`. Gets "not found" error. Retries. Wastes tokens.

### Solution
When a task/project ID is not found exactly, search for close matches.

#### Algorithm
1. Exact match → return immediately
2. If not found, query all IDs with the same prefix (`t-*`, `p-*`)
3. Edit distance (Levenshtein) ≤ 2 → suggest in error message
4. Also search by title substring if input is 3+ chars and doesn't match ID format

#### Error message improvement
Before: `error: task t-a1b3c2 not found`
After: `error: task t-a1b3c2 not found. Did you mean: t-a1b2c3 ("Implement auth")?`

#### Title-based lookup
Allow passing a title substring instead of an ID:
```bash
planq task get "implement auth"    # Fuzzy matches task title
```

If exactly one match → use it. If multiple → show candidates.

Implementation: in `get_task()` and `get_project()`, if exact lookup fails:
1. Load all task IDs in the same project (or recent 100)
2. Compute edit distance
3. If closest match ≤ 2 edits, include in error message
4. If input doesn't look like an ID (no `t-` prefix), try title substring match

#### MCP behavior
Same fuzzy matching. MCP error responses include `"suggestion": "t-a1b2c3"` field so the agent can auto-retry.

---

## Innovation 5: Replan Primitive

### Problem
Agent discovers the plan is wrong mid-execution. Currently needs ~10 calls to replan:
- Cancel 3 tasks (3 calls)
- Create 3 new tasks (3 calls)
- Wire deps (3 calls)  
- Promote (1 call)

### Solution
Single atomic `replan` command:

```bash
planq task replan t-parent --file new-plan.yaml
```

Or via MCP:
```json
{
  "name": "planq_task_replan",
  "arguments": {
    "task_id": "t-parent",
    "cancel_remaining": true,
    "new_subtasks": [
      {"title": "New task A", "description": "..."},
      {"title": "New task B", "deps_on": ["New task A"]}
    ]
  }
}
```

#### Behavior
1. Cancel all non-done subtasks of the parent task (atomic)
2. Create new subtasks with deps (using decompose logic)
3. Promote ready tasks
4. Return new task IDs and updated overview

This is like `decompose` but with a cancel-first step. Replan = "scratch the remaining plan, here's the new one."

#### Keep completed work
Only cancels tasks that are `pending`, `ready`, or `claimed`. Tasks that are `done` or `running` are untouched. This preserves completed work while allowing plan changes.

---

## Innovation 6: Progressive Status

### Problem
`planq project status` and `planq project overview` return too much data for routine "where are we?" checks.

### Solution
Three verbosity levels. Default is the one-liner.

#### Level 0: One-liner (default)
```bash
planq status
```
Output:
```
p-a1b2c3 fibonacci-api: 4/6 done (67%) | ready: t-x1,t-x2 | running: t-x3@agent-1 | blocked: 0
```

That's ~25 tokens. Compare to current overview at ~300+ tokens.

For MCP:
```json
{
  "project": "p-a1b2c3",
  "name": "fibonacci-api",
  "total": 6, "done": 4, "ready": 2, "running": 1, "blocked": 0,
  "progress": "67%",
  "ready_ids": ["t-x1", "t-x2"],
  "running": [{"id": "t-x3", "agent": "agent-1"}]
}
```

#### Level 1: Per-task detail (`--detail`)
```bash
planq status --detail
```
```
p-a1b2c3 fibonacci-api: 4/6 done (67%)
  done  t-a1  Create scaffold
  done  t-b2  Implement core logic  
  done  t-c3  Flask routes
  done  t-d4  Write tests
  ready t-e5  Run integration tests
  ready t-f6  Deploy config
```

#### Level 2: Full dump (`--full`)
```bash
planq status --full
```
Full overview with deps, descriptions, agents, timestamps.

#### Implementation
- New CLI subcommand: `planq status` (not `planq project status`)
- Uses sticky project if `--project` not specified
- Default = one-liner, `--detail` = per-task, `--full` = everything
- New MCP tool: `planq_status` with `detail_level: "summary" | "detail" | "full"`

---

## Innovation 7: Signal/Note System

### Problem
Agents can't communicate except through task completion. But they often discover things mid-execution that other agents need to know.

### Solution
Task notes — append-only messages attached to any task.

```bash
planq task note t-b2c3d4 "Discovered API uses OAuth2, not API keys. Update auth implementation accordingly."
```

#### Schema
```sql
CREATE TABLE IF NOT EXISTS task_notes (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  agent_id TEXT,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL
);
```

#### When notes are surfaced
- In `go` response: notes on the returned task
- In `get_context` response: notes on the task
- In `next` response: notes on the returned task
- Notes are APPEND-ONLY (never edited or deleted — audit trail)

#### MCP tool
```json
{
  "name": "planq_task_note",
  "arguments": {
    "task_id": "t-b2c3d4",
    "content": "Discovered API uses OAuth2",
    "agent_id": "agent-1"
  }
}
```

#### CLI
```bash
planq task note t-b2c3d4 "message here" --agent agent-1
planq task notes t-b2c3d4   # List all notes on a task
```

This enables async agent-to-agent communication through the task graph.

---

## Innovation 8: File-Path Tracking

### Problem
Two parallel agents modify the same files → git merge conflicts when combining work. Nobody knows until merge time.

### Solution
Track files modified per task. Warn on overlaps.

#### On task completion
```bash
planq done t-xxx --result "..." --files src/auth.rs,src/middleware.rs,tests/test_auth.py
```

MCP:
```json
{
  "task_id": "t-xxx",
  "result": "...",
  "files": ["src/auth.rs", "src/middleware.rs"]
}
```

#### Schema
```sql
CREATE TABLE IF NOT EXISTS task_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  path TEXT NOT NULL,
  UNIQUE(task_id, path)
);
```

#### Conflict detection
When `go` or `next` returns a task, check:
1. Get running tasks in the same project
2. Get their file lists
3. If any overlap with the new task's known files (from description or upstream), warn

In response:
```json
{
  "task": {...},
  "file_conflicts": [
    {"path": "src/auth.rs", "also_modified_by": "t-other", "agent": "agent-2", "status": "running"}
  ]
}
```

Even without explicit file tracking, we can mine the `result` field of completed tasks for file paths (look for patterns like `src/*.rs`, `*.py`).

---

## Innovation 9: Partial Completion (Pause/Resume)

### Problem
Agent gets stuck at 70% — maybe needs human approval, or hit an unexpected blocker. Currently: `fail` (loses all progress) or `done` (lies about completion).

### Solution
New `pause` transition: running → paused. Another agent (or same agent later) can resume.

#### State transitions
```
pending → ready → claimed → running → done
                                    → failed
                                    → paused → ready (resume)
```

#### CLI
```bash
planq task pause t-xxx --progress 70 --note "Stuck on OAuth config, need API credentials"
```

#### Behavior
1. Sets status to `paused` (new status)
2. Saves progress percentage and note
3. Releases agent claim (agent_id → null)
4. Task becomes `ready` again (any agent can claim it)
5. When next agent claims, they see the progress note and percentage

Actually, simpler: `pause` sets status back to `ready` with progress/note preserved. No new status needed.

```bash
planq task pause t-xxx --progress 70 --note "Need OAuth creds"
# Status: running → ready (with progress=70, note="Need OAuth creds")
```

When next agent does `planq go`:
```json
{
  "task": {
    "id": "t-xxx",
    "title": "Implement auth",
    "status": "running",
    "progress": 70,
    "progress_note": "Need OAuth creds",
    "previous_agent": "agent-1"
  }
}
```

---

## Innovation 10: (Bonus) LLM System Prompt Template

Ship a ready-to-use system prompt snippet that agents can use:

```markdown
## Task Management
You have access to Planq for task management. Quick reference:
- `planq go --agent <you>` — get next task (auto claim+start) with upstream context
- `planq done <id> --result "what you did" --files f1,f2 --next` — complete + get next
- `planq status` — one-line project progress
- `planq task note <id> "message"` — leave notes for other agents
- `planq task pause <id> --note "why"` — release task if stuck
- `planq task replan <parent> --file plan.yaml` — change the plan

IDs are short (8 chars): t-a1b2c3, p-x4y5z6
Default project is set automatically. No need for --project.
```

---

## Implementation Priority Order

1. **Compound commands** (go, done --next) — biggest bang, 60% fewer round-trips
2. **Sticky project** — removes ~15 tokens from every call
3. **Progressive status** — the "quick check" goes from 300 to 25 tokens
4. **Handoff protocol** — agents actually understand upstream context
5. **Signal/note system** — inter-agent communication
6. **Fuzzy ID resolution** — error recovery (reduces wasted retries)
7. **Replan primitive** — JIT planning enabler
8. **Partial completion** — stuck agent recovery
9. **File-path tracking** — parallel work safety net

## Schema Changes Required

```sql
-- New tables
CREATE TABLE IF NOT EXISTS planq_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS task_notes (
  id TEXT PRIMARY KEY,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  agent_id TEXT,
  content TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS task_files (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id TEXT NOT NULL REFERENCES tasks(id),
  path TEXT NOT NULL,
  UNIQUE(task_id, path)
);
```

No existing table modifications. All additive.

## Token Budget Analysis (After All Innovations)

| Operation | Before | After | Savings |
|-----------|--------|-------|---------|
| Get + claim + start task | 150 tokens | 30 tokens (go) | 5x |
| Complete + check status + get next | 180 tokens | 30 tokens (done --next) | 6x |
| Check project status | 300 tokens | 25 tokens (status one-liner) | 12x |
| Full inner loop per task | 280 tokens | 60 tokens | 4.7x |
| 20-task project overhead | 5,600 tokens | 1,200 tokens | 4.7x |
| ID per mention | 32 chars | 8 chars | 4x |
