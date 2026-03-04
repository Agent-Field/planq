# Planq

The task DAG primitive for AI agent orchestration.

![Version](https://img.shields.io/badge/version-0.1.0-blue)
![License](https://img.shields.io/badge/license-Apache%202.0-green)
![Rust](https://img.shields.io/badge/rust-stable-orange)
![Binary Size](https://img.shields.io/badge/binary%20size-~2.6MB-informational)
![Works with Claude Code](https://img.shields.io/badge/works%20with-Claude%20Code-5A67D8)
![Works with Codex](https://img.shields.io/badge/works%20with-Codex-111111)
![Works with MCP](https://img.shields.io/badge/works%20with-any%20MCP%20client-1f6feb)

Planq exists for teams running real parallel agent work, where issue trackers become coordination overhead. It is one Rust binary with zero runtime dependencies and three interfaces: CLI, MCP server, and HTTP API. It is local-first and SQLite-backed, so the database is the API and handoff medium. The surface area is tuned for LLMs: 8-char IDs, compact output, fuzzy ID resolution, and AI-native commands like `go` and `done --next`.

## Quick Start

```bash
cargo install --path .
planq project create "ship-planq"
planq task create --project <PROJECT_ID> --title "Design API"
planq go --project <PROJECT_ID> --agent claude-1
planq done --next
```

## The Agent Loop

```bash
planq go --project <PROJECT_ID> --agent <AGENT_NAME>
planq done --next
```

## Install

```bash
# From source
cargo install --path .

# Or use a release binary
curl -L https://github.com/<org>/planq/releases/latest/download/planq-$(uname -s)-$(uname -m) -o planq
chmod +x planq && mv planq /usr/local/bin/planq
```

## MCP Config

```json
{
  "mcpServers": {
    "planq": {
      "command": "planq",
      "args": ["mcp"]
    }
  }
}
```

Use the same block in Claude Code, Cursor, or any MCP client config.

## Features

- Short IDs: 8-char task/project IDs built for token budgets
- DAG dependencies: explicit `feeds_into`, `blocks`, `validates`, `informs`
- Claim protocol: atomic claim + heartbeat + timeout reclaim
- Compound commands: `go`, `done --next`, `next --claim`
- Compact output: terse default formatting for context windows
- Handoff protocol: agent-safe transitions across sessions
- Fuzzy IDs: resolve partial IDs without copy/paste churn
- File tracking: attach changed files and artifacts to task state
- Signals: event stream for orchestration and dashboards
- Three interfaces: CLI, MCP server, HTTP API on one SQLite core
- Single binary: local-first, no daemon, no external services

## Comparison

| Tool | Local-first | MCP | CLI | HTTP | DAG deps | Token-optimized | Single binary |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Planq | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| GitHub Issues | No | No | Limited | Yes | No | No | No |
| Linear | No | No | Limited | Yes | No | No | No |
| TaskMaster AI | No | Partial | Yes | No | Partial | Partial | No |
| Beads | Yes | No | Yes | No | Partial | No | Yes |

## Philosophy

Planq is a primitive, not a platform. It does one job: coordinate dependent work across humans and agents with minimal protocol overhead. Keep orchestration local, inspectable, and scriptable.

## Topics

`ai-agents` `task-management` `dag` `mcp` `cli` `rust` `sqlite` `llm` `orchestration` `ai-native`

## License

Apache License 2.0
