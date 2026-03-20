#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# PlanDB Example: Build Documentation Site + Interactive Playground
#
# Demonstrates PlanDB's compound graph features by having Codex CLI
# use PlanDB to plan, decompose, and execute a multi-phase project.
#
# Usage:
#   ./examples/build-docs-site.sh          # interactive (see the TUI)
#   ./examples/build-docs-site.sh --exec   # non-interactive (headless)
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLANDB_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_DIR="$PLANDB_ROOT/docs-site"

# Check dependencies
command -v plandb >/dev/null 2>&1 || { echo "error: plandb not found in PATH. Run: cargo install --path $PLANDB_ROOT"; exit 1; }
command -v codex  >/dev/null 2>&1 || { echo "error: codex not found in PATH. Install: https://github.com/openai/codex"; exit 1; }

# Clean slate
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"
export PLANDB_DB="$WORK_DIR/.plandb.db"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  PlanDB Example: Docs Site + Playground                     ║"
echo "║  Codex will use PlanDB to plan and build the project.       ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# ── Step 1: Seed the task graph ──────────────────────────────────────
# We create top-level tasks. Codex will split them further as it works.

plandb init "plandb-docs-site"

plandb add "Research and design site structure" \
  --as design --kind research \
  --description "Decide on tech stack (vanilla HTML/CSS/JS preferred, or lightweight like Vite). Plan page layout, navigation, color scheme. Output: a brief design doc."

plandb add "Build documentation pages" \
  --as docs --kind code --dep t-design \
  --description "Create HTML pages: landing/hero, getting-started guide, CLI reference (generate from plandb --help and subcommands), architecture page (compound graph model). Use clean, minimal design."

plandb add "Build interactive playground" \
  --as playground --kind code --dep t-design \
  --description "Create a terminal-like UI in the browser where users can try PlanDB commands. Use simulated/pre-recorded output. Include a guided tutorial: init → add → go → split → done → status. Should feel like a real terminal."

plandb add "Generate CLI reference content" \
  --as cli-ref --kind shell --dep t-design \
  --description "Run plandb --help, plandb task --help, plandb split --help, etc. Capture all output. Format as structured HTML reference. Include every command, flag, and example."

plandb add "Polish, integrate, and style" \
  --as polish --kind code \
  --dep t-docs --dep t-playground --dep t-cli-ref \
  --description "Consistent styling across all pages. Navigation links work. Responsive layout. Add favicon, meta tags. Ensure playground is embedded in docs."

plandb add "Deploy configuration" \
  --as deploy --kind code --dep t-polish \
  --description "Add GitHub Pages config, a simple serve script, and a README for the docs-site directory."

echo ""
echo "=== Task Graph ==="
plandb status --detail
echo ""

# ── Step 2: Capture plandb help for Codex context ────────────────────
PLANDB_HELP=$(plandb --help 2>&1)
PLANDB_PROMPT=$(plandb prompt --for cli 2>&1 | head -60)

# ── Step 3: Build the Codex prompt ───────────────────────────────────
PROMPT=$(cat <<PROMPT_EOF
You are building a documentation site and interactive playground for PlanDB.

## What is PlanDB
PlanDB is a CLI task graph for AI agents. It manages dependency-aware task graphs in SQLite.
Core loop: \`plandb go\` → work → \`plandb done --next\`. Tasks can be recursively split.

## Your workflow
A PlanDB project is already initialized with 6 tasks. Your job:

1. Run \`plandb status --detail\` to see the task graph
2. Run \`plandb go\` to claim the next ready task
3. Read the task description with \`plandb show <id>\`
4. Do the work (create files, write code)
5. If the task is complex, split it: \`plandb split --into "Part A > Part B > Part C"\`
6. Run \`plandb done --next\` to complete and claim the next task
7. Repeat until \`plandb status\` shows 100%

IMPORTANT: Use plandb commands throughout — this IS the demo of PlanDB.

## PlanDB commands you'll need
\`\`\`
plandb go                              # claim next ready task
plandb done --next                     # complete current + claim next
plandb done --result '{"key":"val"}'   # complete with result data
plandb split --into "A, B, C"          # split into independent parts
plandb split --into "A > B > C"        # split into dependency chain
plandb status --detail                 # check progress
plandb show <task-id>                  # see full task description
plandb add "title" --dep t-xxx         # add a new task mid-flight
\`\`\`

## What to build
The task descriptions have the details. In summary:
- Static HTML/CSS/JS documentation site (no heavy frameworks)
- Interactive terminal playground (simulated PlanDB commands in browser)
- CLI reference generated from actual \`plandb --help\` output
- Clean, minimal, professional design
- All files in the current working directory

## plandb --help output (for CLI reference page)
\`\`\`
${PLANDB_HELP}
\`\`\`

Start by running \`plandb status --detail\` to see your tasks, then \`plandb go\` to begin.
PROMPT_EOF
)

# ── Step 4: Launch Codex ─────────────────────────────────────────────
MODE="${1:---interactive}"

if [[ "$MODE" == "--exec" ]]; then
  echo "Running in non-interactive (exec) mode..."
  codex exec --full-auto -C "$WORK_DIR" "$PROMPT"
else
  echo "Launching interactive Codex session..."
  echo "(You'll see the TUI — watch Codex use PlanDB to plan and execute)"
  echo ""
  codex --full-auto -C "$WORK_DIR" "$PROMPT"
fi

# ── Step 5: Show final status ────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Build Complete                                             ║"
echo "╚══════════════════════════════════════════════════════════════╝"
plandb status --detail
echo ""
echo "Output: $WORK_DIR"
echo "Serve:  cd $WORK_DIR && python3 -m http.server 8080"
