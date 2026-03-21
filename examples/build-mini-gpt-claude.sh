#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# PlanDB Example: Build Mini GPT from Scratch in Rust
#
# Claude Code builds a working transformer language model in pure Rust
# (no ML framework deps) using PlanDB for task orchestration.
#
# This is a hard test: genuine 3+ level hierarchy, cross-level deps,
# parallel implementation tracks, quality gates, mid-flight adaptation.
#
# Usage:
#   ./examples/build-mini-gpt-claude.sh             # interactive
#   ./examples/build-mini-gpt-claude.sh --headless   # non-interactive
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PLANDB_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_DIR="$PLANDB_ROOT/mini-gpt-rust"

command -v plandb >/dev/null 2>&1 || { echo "error: plandb not found. Run: cargo install --path $PLANDB_ROOT"; exit 1; }
command -v claude >/dev/null 2>&1 || { echo "error: claude not found. Install: https://docs.anthropic.com/en/docs/claude-code"; exit 1; }

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  PlanDB Example: Mini GPT in Rust (Claude Code)            ║"
echo "║  From-scratch transformer with PlanDB task orchestration.   ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Load shared PlanDB reference
PLANDB_REF=$(cat "$SCRIPT_DIR/PLANDB_PROMPT.md")

SYSTEM_PROMPT="${PLANDB_REF}

## Claude Code: Parallel Execution

You have the Agent tool. After creating the task graph:

1. Run \`plandb list --status ready\` to find all parallelizable tasks
2. If multiple are ready, spawn one Agent per task IN PARALLEL (single message, multiple Agent tool calls)
3. Each agent: \`PLANDB_AGENT=worker-N plandb go\` → \`plandb show <id>\` → implement → \`PLANDB_AGENT=worker-N plandb done\`
4. After agents finish, \`plandb status --detail\` + \`plandb critical-path\` → repeat
5. Continue until 100%

DO NOT work sequentially when tasks could be parallel."

export PLANDB_DB="$WORK_DIR/.plandb.db"
MODE="${1:---interactive}"

KICKOFF=$(cat <<KICKOFF_EOF
## Task: Build a Mini GPT from Scratch in Rust

Build a **working transformer language model** in pure Rust. No ML framework crates.
After \`cargo run\`, it trains on text and generates coherent output.

### Hard Constraints
- **No ML deps**: no tch, candle, burn, ndarray. Only \`rand\` crate allowed.
- **Single binary**: \`cargo run\` trains the model and generates text.
- **Character-level tokenizer** (like Karpathy's nanoGPT char-level).
- **Bundled training data**: embed a small text corpus (~50KB) as a string constant or include_str!. Use a Shakespeare excerpt or similar public domain text.
- **Working output**: after training (~30-60 seconds), generate 500 characters that show learned language patterns (not random garbage).

### Model Spec
- Embedding dim: 64-128
- Attention heads: 4
- Transformer layers: 2-4
- Context window: 64-128 tokens
- Vocab: all unique chars in the training text (~65 for Shakespeare)

### What to Implement (bottom-up)

**Layer 1 — Tensor foundations (no autograd yet, just forward ops):**
- \`Tensor\` struct: 2D f32 matrix with shape tracking
- Ops: matmul, transpose, add, element-wise multiply, softmax, layer_norm
- Activations: GELU or ReLU

**Layer 2 — Neural network building blocks:**
- \`Linear\` layer: weight matrix + bias, forward pass
- \`Embedding\` layer: lookup table (vocab_size x embed_dim)
- \`LayerNorm\`: normalize across features

**Layer 3 — Transformer components:**
- Scaled dot-product attention with causal mask
- Multi-head attention (split heads, attend, concat, project)
- Feed-forward network (Linear → GELU → Linear)
- Residual connections

**Layer 4 — Full model:**
- \`TransformerBlock\`: LayerNorm → MultiHeadAttention → residual → LayerNorm → FFN → residual
- \`GPT\`: token embedding + position embedding → N transformer blocks → LayerNorm → linear projection to vocab
- Forward pass: input token IDs → logits over vocab

**Layer 5 — Training:**
- Cross-entropy loss (manual implementation)
- **Backpropagation**: this is the hardest part. Implement manual gradient computation for each operation. Use a simple approach: store intermediate values during forward pass, compute gradients layer by layer in reverse.
- Optimizer: SGD with learning rate (Adam is better but SGD works for a demo)
- Training loop: sample random windows from text, forward, loss, backward, update weights
- Print loss every N steps to show it's decreasing

**Layer 6 — Inference:**
- Temperature-based sampling from output logits
- Generate character by character, feeding output back as input
- Start from a seed string

### Quality Expectations
- \`cargo build\` must compile with zero errors
- Training loss must visibly decrease over epochs
- Generated text should contain recognizable English words/patterns (not random chars)
- Code must be well-organized: one file per module (tensor.rs, attention.rs, model.rs, train.rs, etc.)

### Using PlanDB

You are in $(pwd). PLANDB_DB=$PLANDB_DB is set. Run plandb commands directly.

1. \`plandb init "mini-gpt-rust"\`
2. Decompose this into tasks with dependencies and detailed descriptions. Use the layer structure above as a guide but decompose further where needed.
3. Use \`--pre\` and \`--post\` conditions. Example: \`--post "cargo build compiles with no errors"\`
4. Use \`plandb critical-path\` to prioritize. Tensor ops are on the critical path — everything depends on them.
5. Parallelize independent implementations (attention + FFN + layernorm can be parallel once tensor ops exist).
6. If backprop is too complex in one task, split it: \`plandb split --into "Linear backward > Attention backward > FFN backward > Full backward pass"\`

### CRITICAL: Verification Loop

You are NOT done until the model works end-to-end. After implementing everything:

1. Run \`cargo build\` — fix ALL compile errors. Do not move on until it compiles cleanly.
2. Run \`cargo run\` — the program must train and generate text.
3. **Check the output**: training loss must decrease over steps. Generated text must contain recognizable English words, not random characters.
4. If it doesn't compile: read the errors, fix them, rebuild. Repeat until clean.
5. If it compiles but crashes at runtime: debug, fix, rerun. Repeat until it runs to completion.
6. If it runs but generates garbage: the model or training has a bug. Check gradient computation, learning rate, loss function. Fix and retrain.
7. If loss doesn't decrease: learning rate may be wrong, gradients may be zero, or the backward pass has a bug. Debug and fix.

**Keep iterating until \`cargo run\` produces output where you can read English words in the generated text.** This is your exit condition — not "I wrote all the code." The code must WORK.

Add a final task in PlanDB: \`plandb add "Verify end-to-end" --post "cargo run compiles, trains with decreasing loss, and generates recognizable English text" --description "Run cargo run. Verify: 1) compiles 2) loss decreases 3) generated text has English words. If any fail, debug and fix until all three pass."\`

Start now.
KICKOFF_EOF
)

if [[ "$MODE" == "--headless" ]]; then
  echo "Running in headless mode..."
  claude -p \
    --allowedTools "Bash Agent Read Write Edit Grep Glob" \
    --permission-mode "auto" \
    --model sonnet \
    --system-prompt "$SYSTEM_PROMPT" \
    "$KICKOFF"
else
  echo "Launching Claude Code interactive session..."
  echo ""
  cd "$WORK_DIR"
  claude \
    --permission-mode auto \
    --allowedTools "Bash Edit Write Read Grep Glob Agent" \
    --system-prompt "$SYSTEM_PROMPT" \
    "$KICKOFF"
fi

echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  Build Complete                                             ║"
echo "╚══════════════════════════════════════════════════════════════╝"
plandb status --full 2>/dev/null || echo "(no plandb project found)"
echo ""
echo "Output: $WORK_DIR"
echo "Run:    cd $WORK_DIR && cargo run"
