#!/usr/bin/env bash
# Fetch the LLM for the `llm` feature (PLAN.md M1, src/llm.rs).
#
# Ported from LocalGPT Verse's scripts/fetch-bonsai.sh (Apache-2.0); the model
# and the runtime constraints are the same ones Verse verified on Apple
# Silicon (2026-09):
#   - Bonsai-8B Q4_K_M (~5.2 GB, Apache-2.0) runs via mistral.rs under the
#     `llm-metal` feature. The 1-bit Q1_0 quant does NOT parse in mistral.rs
#     0.8 — don't fetch it.
#   - The 5 GB Q4_K_M does not fit the CPU device map alongside the renderer
#     on a 32 GB Mac — build with `--features llm-metal` on macOS.
#   - mistral.rs 0.8's grammar-constrained `generate_structured` hangs on GGUF
#     — src/llm.rs deliberately uses plain instructed-JSON generation instead.
#
# The tokenizer comes from prism-ml/Bonsai-8B-unpacked (the GGUF repos ship
# none, and mistral.rs's GgufModelBuilder requires one).
#
# Any standard GGUF (e.g. Qwen2.5-7B-Instruct) + its matching tokenizer.json
# dropped into assets/llm/ is picked up the same way — src/llm.rs loads the
# first .gguf it finds, so no code change is needed.
#
# Shared model directory: the model goes to ~/.local/share/localgpt/models/llm
# ($LOCALGPT_LLM_DIR overrides; $XDG_DATA_HOME moves the base), the directory
# LocalGPT MD, Verse and Gen all read, so one ~5 GB download serves every app.
# A copy already in this repo's assets/llm/ (where this script used to put it)
# is moved there instead of being downloaded again.
#
# Newer Bonsai generations exist (Ternary-Bonsai-2-27B, 2026-09), but the
# ternary quants are the class that failed to parse in mistral.rs 0.8, and
# this 8B Q4_K_M is the configuration Verse runtime-verified. Override with
# BONSAI_REPO / BONSAI_FILE / BONSAI_TOKENIZER_REPO to try another model —
# GGUF_FILE must be a bare filename (no directory part).
set -euo pipefail
cd "$(dirname "$0")/.."

# Keep in step with src/llm.rs `shared_llm_dir` (and localgpt-core's).
OUT="${LOCALGPT_LLM_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/localgpt/models/llm}"
LEGACY="assets/llm"
mkdir -p "$OUT"

# Defaults; override with BONSAI_REPO / BONSAI_FILE / BONSAI_TOKENIZER_REPO.
GGUF_REPO="${BONSAI_REPO:-bartowski/prism-ml_Bonsai-8B-unpacked-GGUF}"
GGUF_FILE="${BONSAI_FILE:-prism-ml_Bonsai-8B-unpacked-Q4_K_M.gguf}"
TOK_REPO="${BONSAI_TOKENIZER_REPO:-prism-ml/Bonsai-8B-unpacked}"

fetch() { # <label> <url> <dest>
  local label="$1" url="$2" dest="$3"
  local legacy="$LEGACY/$(basename "$dest")"
  if [ -s "$dest" ]; then
    echo "  have $dest"
  elif [ -s "$legacy" ]; then
    echo "moving $legacy -> $dest (shared with the other LocalGPT apps)"
    mv "$legacy" "$dest"
  else
    echo "fetching $label -> $dest (resumable — re-run if interrupted)"
    # -C - resumes a partial download; -L follows HF redirects.
    curl -L --fail -C - -o "$dest" "$url"
  fi
}

fetch "GGUF ($GGUF_REPO)" \
  "https://huggingface.co/${GGUF_REPO}/resolve/main/${GGUF_FILE}" \
  "$OUT/$GGUF_FILE"
fetch "tokenizer ($TOK_REPO)" \
  "https://huggingface.co/${TOK_REPO}/resolve/main/tokenizer.json" \
  "$OUT/tokenizer.json"

echo "done -> $OUT"
echo "license: Bonsai-8B weights + tokenizer are Apache-2.0 (prism-ml /"
echo "        bartowski's quant); still verify NOTICE.txt before distribution."
echo "run with: cargo run --features llm-metal -- samples/hello.md --generate"
