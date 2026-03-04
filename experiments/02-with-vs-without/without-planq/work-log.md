# quicknote work log

## 1) Initial setup and discovery
- Verified working directory contents first.
- Found the directory was empty, so created project structure from scratch.

## 2) Test-first workflow
- Wrote pytest CLI tests first in `tests/test_cli.py` for:
  - `add`
  - `list` (newest first)
  - `search`
  - `tag`
  - `list --tag`
  - `export --format json`
  - `stats`
- Ran tests immediately after writing them to confirm an initial failure.
- Initial failure was expected (`ModuleNotFoundError: No module named 'quicknote'`).

## 3) Implementation work
- Created package files:
  - `quicknote/__init__.py`
  - `quicknote/db.py`
  - `quicknote/cli.py`
- Implemented SQLite schema and behavior:
  - `notes` table
  - `tags` table
  - `note_tags` join table
  - FTS5 virtual table (`notes_fts`) for note content search
  - FTS sync triggers for insert/update/delete
- Implemented CLI commands with `click`:
  - `quicknote add "..."`
  - `quicknote list`
  - `quicknote search "..."`
  - `quicknote tag <id> <tag>`
  - `quicknote list --tag <tag>`
  - `quicknote export --format json`
  - `quicknote stats`

## 4) Debug/fix cycle
- Ran tests after initial implementation.
- One failing test: search did not match partial term (`arch` vs `architecture`).
- Updated search behavior to build prefix FTS query terms (`term*`) so partial keyword search passes.
- Re-ran tests and confirmed all tests pass.

## 5) Packaging
- Added `pyproject.toml` with:
  - build-system via setuptools
  - project metadata
  - dependency on `click`
  - console script entry point: `quicknote = quicknote.cli:cli`
  - pytest configuration (`testpaths = ["tests"]`)

## 6) Type/lint quality pass
- Ran LSP diagnostics on changed Python files.
- Cleaned diagnostics by adding explicit type aliases (`TypedDict`) and casts around sqlite row access.
- Removed/avoided unnecessary comments/docstrings.

## 7) Final test results
- Command requested: `python -m pytest -v`
  - Result in this environment: `python` command not available (`command not found`).
- Equivalent run with available interpreter: `python3 -m pytest -v`
  - Result: **5 passed**.
  - Test file: `tests/test_cli.py`
  - Note: external environment warnings from installed pytest plugins were present, but tests passed.
