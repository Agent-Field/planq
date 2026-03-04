# Experiment: With vs Without Planq

## Hypothesis
An AI agent given a complex multi-step task will produce more structured, 
complete work when equipped with Planq for task decomposition and tracking.

## Task (identical for both runs)
Build a Python CLI tool called `quicknote` — a local note-taking app.

Requirements:
1. `quicknote add "my note"` — adds timestamped note to SQLite DB
2. `quicknote list` — shows all notes, newest first
3. `quicknote search "keyword"` — full-text search
4. `quicknote tag <id> <tag>` — add tag to a note
5. `quicknote list --tag work` — filter by tag
6. `quicknote export --format json` — export all notes as JSON
7. `quicknote stats` — show note count, tag distribution, date histogram
8. Unit tests with pytest covering all commands
9. A proper `setup.py` or `pyproject.toml` so it installs as a CLI

This task is complex enough (9 requirements, multiple files, natural dependencies)
that planning matters, but simple enough for a single agent session.

## What we're measuring
- Did the agent decompose the task before starting?
- Did it track what's done vs remaining?
- Did it miss any of the 9 requirements?
- How many times did it lose track or repeat work?
- Final completeness: what % of requirements were delivered?

## Prompt sizes
- WITHOUT Planq: just the task (~150 tokens)
- WITH Planq: task + 8-line Planq reference (~210 tokens)
  - Overhead of Planq prompt: ~60 tokens (trivial)
