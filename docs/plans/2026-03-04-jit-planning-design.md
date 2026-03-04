# Planq JIT Adaptive Planning — Design Spec

**Date**: 2026-03-04  
**Status**: Design  
**Principle**: Simple mutations + rich feedback = powerful planning

---

## The Problem

Agent is on task 5 of 12. It discovers the plan is wrong.

**Current state**: Fixing a plan requires ~15 CLI calls (cancel tasks, create new ones, wire deps, check status). By the time the agent is done replanning, it's burned 3,000+ tokens and probably made a mistake in the graph wiring.

**Target state**: Any plan change is 1-3 calls. Every response shows the ripple effect. The agent never needs to "check what happened" — the response tells it.

---

## Design Principle: Mutation + Feedback

LLMs are bad at complex graph operations. They're good at:
- Simple, local changes ("add this", "remove that", "swap these")
- Reading structured feedback ("3 tasks delayed, 1 task now ready")

So: **keep mutations dead simple, make feedback rich.**

Every mutation response follows the same schema:
```json
{
  "action": "insert|cancel|pivot|split|amend",
  "created": [],
  "cancelled": [],
  "modified": [],
  "effect": {
    "delayed": [],
    "accelerated": [],
    "ready_now": [],
    "blocked_now": [],
    "critical_path": []
  },
  "project_state": {
    "total": 12, "done": 5, "ready": 2, "running": 1, "pending": 4
  }
}
```

**Key insight**: project_state is included in EVERY mutation response. The agent never needs a separate status call after a plan change.

---

## The 6 JIT Primitives

### 1. `what-if` — Preview before committing

Dry-run any mutation. Returns the same effect schema but makes no changes.

**CLI**:
```bash
planq what-if cancel t-b2
planq what-if insert --after t-a1 --before t-b2 --title "new step"
```

**MCP**: `planq_what_if` with `mutation` object describing the change.

**Response**:
```json
{
  "safe": true,
  "would_cancel": ["t-b2", "t-d4"],
  "would_become_ready": ["t-f6"],
  "would_orphan": [],
  "critical_path_change": false
}
```

**Why this matters**: The agent can reason about consequences before acting. Eliminates "oops, I cancelled too much" cascades.

### 2. `insert` — Add a task between existing tasks

```bash
planq task insert --after t-a1 --before t-b2 --title "Add OAuth"
```

**Semantics**:
1. Create new task X
2. Remove edge A → B
3. Add edges A → X and X → B
4. If B was ready and A is not done, B becomes pending

Single atomic operation. The `--before` flag is optional — without it, X is just added after A.

**MCP**: `planq_task_insert` with `after_task`, `before_task` (optional), `title`, `description`.

### 3. `pivot` — Replace a subtree

```bash
planq task pivot t-parent --keep-done --subtasks '[
  {"title": "New approach A"},
  {"title": "New approach B", "deps_on": ["New approach A"]}
]'
```

Or from file: `planq task pivot t-parent --keep-done --file new-plan.yaml`

**Semantics**:
1. Find all children of t-parent
2. Cancel children that are pending/ready/claimed (not done, not running)
3. If `--keep-done`: preserve completed children
4. If a child is running: error (must wait or pause it first)
5. Create new subtasks with deps
6. Return: what was kept, what was cancelled, what was created

### 4. `ahead` — Lookahead buffer

```bash
planq ahead --depth 2
```

Shows the currently running task + tasks 1-N hops downstream.

**Response**:
```json
{
  "current": {"id": "t-b1", "title": "Implement add", "status": "running"},
  "upcoming": [
    {"id": "t-b2", "title": "Implement list", "hops": 1, "blocked_by": ["t-b1"], "description": "..."},
    {"id": "t-b3", "title": "Search", "hops": 1, "blocked_by": ["t-b1"]},
    {"id": "t-c1", "title": "Tags", "hops": 2, "blocked_by": ["t-b1"]}
  ],
  "updatable": ["t-b2", "t-b3", "t-c1"]
}
```

**Why this matters**: The agent sees what's coming. It can update future task descriptions with context it's learning NOW. This creates a "rolling planning horizon" — the plan refines itself as work progresses.

### 5. `amend` — Accumulate context on future tasks

```bash
planq task amend t-b2 --prepend "NOTE: DB uses 'notes' table (id INTEGER, content TEXT, created_at TEXT). Use this schema."
```

**Semantics**: Prepends text to the task's description, separated by `\n---\n`. Does NOT replace — accumulates. Each amendment is timestamped.

**Why this matters**: As agents learn things, they can annotate future tasks. When another agent picks up that task, it has all the context. This turns the task DAG into a **knowledge propagation network**.

### 6. `split` — Decompose mid-execution

Agent is working on a task and realizes it's too big:

```bash
planq task split t-b1 --into '[
  {"title": "Create DB schema", "done": true, "result": "Created notes table with id, content, created_at"},
  {"title": "Implement add function"},
  {"title": "Wire CLI command", "deps_on": ["Implement add function"]}
]'
```

**Key difference from decompose**: subtasks can be pre-marked as `done` with a `result`. The agent captures work it already completed within the parent task.

The parent task status changes to composite. The overall DAG adjusts.

---

## Hierarchical JIT Planning

The nested planning model uses existing `parent_task_id` + `is_composite`:

```
Project: quicknote
  Phase: Foundation [composite, done]
    t-a1: DB schema [done]
    t-a2: Project structure [done]
  Phase: Core [composite, running]         <- currently here
    t-b1: Add command [done]
    t-b2: List command [running]
    t-b3: Search [ready]
  Phase: Advanced [composite, pending]     <- not planned yet
  Phase: Quality [composite, pending]      <- not planned yet
```

**The rule**: Only plan phases 1-2 hops ahead. Leave distant phases as empty composites. When you get there, decompose them.

### `plan-next` — JIT phase decomposition

```bash
planq task plan-next t-advanced-phase --subtasks '[...]'
```

Only works on composite tasks with no children. It's the "now it's time to plan this" trigger.

This creates a natural **plan-as-you-go** workflow:
1. Create project with high-level phases (5 min)
2. Detail only Phase 1 (2 min)
3. Start working
4. When Phase 1 is ~done, detail Phase 2 based on what you learned
5. Repeat

---

## Effect Analysis Engine

The core of the feedback system. Every mutation runs through this before returning.

### Inputs
- The mutation (what changed)
- The current DAG state

### Outputs
```rust
pub struct MutationEffect {
    pub delayed: Vec<String>,       // tasks whose earliest-start shifted later
    pub accelerated: Vec<String>,   // tasks that can now start sooner  
    pub ready_now: Vec<String>,     // tasks that just became ready
    pub blocked_now: Vec<String>,   // tasks that just became blocked
    pub critical_path: Vec<String>, // longest chain from any ready to any leaf
    pub depth: usize,               // depth of the DAG
}
```

### Implementation
After any mutation:
1. Run BFS from all ready/pending tasks
2. Compute reachability sets
3. Compare before/after to determine delayed vs accelerated
4. Find critical path via longest-path in DAG
5. Return the diff

This is O(V+E) — fast for any reasonable project size (<1000 tasks).

---

## What NOT to Build

| Tempting idea | Why not |
|--------------|---------|
| Conditional branching (if/else in DAG) | LLMs can't reason about branching reliably. Use `what-if` + manual choice instead. |
| Automatic replanning | Planq should never change the plan autonomously. Agency belongs to the agent. |
| Time estimation | LLM agents don't reason about time. Adds complexity with no value. |
| Hypergraph edges | A DAG with composites is equivalent in expressiveness and much simpler to reason about. |
| Undo/redo | Mutations are logged in events table. For recovery, use `what-if` to preview, then commit. |

---

## Prompt Addition for JIT Planning

The entire JIT system adds ~40 tokens to the agent's system prompt:

```
Adapt your plan mid-execution:
  planq what-if cancel ID         — preview effects
  planq task insert --after A --before B --title "..."
  planq ahead --depth 2           — see upcoming tasks
  planq task pivot PARENT --file new-plan.yaml
Only plan 1-2 phases ahead. Refine as you learn.
```

---

## Implementation Priority

1. **what-if** — highest value, enables safe plan changes
2. **insert** — most common mutation (add a step between)
3. **ahead** — makes the agent proactive, not reactive
4. **pivot** — handles "the approach is wrong" scenario
5. **amend** — context accumulation for multi-agent
6. **split** — mid-task decomposition
7. **Effect analysis engine** — powers all feedback

---

## Token Budget

| Operation | Tokens |
|-----------|--------|
| `what-if cancel` response | ~30 |
| `insert` response with effects | ~50 |
| `ahead --depth 2` response | ~60 |
| `pivot` response | ~40 |
| Agent prompt overhead | ~40 |

Total JIT planning overhead per plan change: **~50 tokens average** (vs ~3,000 tokens for manual graph surgery today).
