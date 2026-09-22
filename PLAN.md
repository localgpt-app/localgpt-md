# LocalGPT MD — Plan

Open a Markdown file and walk through it as a 3D world. Verse turns a song
into a world; this turns a document into one. Most of the machinery already
exists in LocalGPT Verse, so this plan is mostly about porting it behind a
Markdown front end.

## M0 — Scaffold ✅

- Markdown → sections with BLAKE3 hashes (`doc.rs`).
- Rule-based draft → a `localgpt-world-types` `WorldManifest` (`draft.rs`),
  checked with Gen's validation.
- Bevy renderer using Gen's mapping (`scene.rs`); tour navigation and caption
  (`tour.rs`).
- Hot reload by polling the file (`watch.rs`); `--print-ron`; an offscreen
  screenshot mode for checking the render without a display.

## M1 — Recipe tier: a local LLM per section ✅

- Ported Verse's `llm.rs` and the recipe idea behind `llm` / `llm-metal`
  features (mistral.rs 0.8; Bonsai-8B Q4_K_M via `scripts/fetch-bonsai.sh`,
  which also reuses a sibling Verse checkout's model). Verse's `tier.rs`
  became a simpler lazy load inside the worker (load on first job, keep
  resident).
- Prompt per section: heading + body excerpt + genre → a `RegionRecipe`
  (palette, landmark kind/scale/glow, prop kind/count), clamped on the Rust
  side. Any failure keeps the draft; failures are sticky for the run.
- The draft renders instantly; a background `std::thread` worker (Verse's
  analysis-worker split, mpsc channels) styles uncached sections and each
  region upgrades in place as its recipe lands. `--generate` runs the same
  authoring headless.
- Verse's constraints carried over verbatim (`ARCHITECTURE.md` §10): plain
  generation with a lenient JSON parse (grammar-constrained generation hangs
  on GGUF), no `schemars` dependency, Metal for the 5 GB quant.
- Deliberately not in the recipe yet: atmosphere (fog/ambient are global in
  the manifest — needs a per-region tint mechanism first).

## M2 — Section cache (the "lockfile") ✅

- `src/sidecar.rs`: `<doc>.world.json`, keyed by BLAKE3 of heading + body,
  pruned to live sections on save, written atomically, clamped on load.
  Unchanged sections never regenerate; a `.md` + sidecar renders identically
  in any build (the default build applies cached recipes, it just can't
  author them).
- `meta.source` says "draft + llm recipe" when any recipe is applied.
  `meta.model` still isn't set: the sidecar doesn't record which model
  authored a recipe. Add a `model` field in sidecar v2 when a second model
  becomes an option (Bonsai-8B vs stock Qwen3-8B are drop-in `--generate`
  A/B candidates; the ternary Bonsai generations can't run on mistral.rs).

## M3 — Agent tier

- Port Verse's `agent.rs` and `agent_types.rs` (itself a trimmed port of
  Gen's `gen3d` tool loop): the model builds a region through tool calls and
  emits `localgpt-world-types` entities.
- Place real assets from Verse's asset pack (`localgpt-verse-assets`).
- Fenced ```` ```world ```` blocks in a section act as exact overrides.

## M4 — Genres

- `genre: deck` first: `---`-separated slides become tour stops and the LLM
  stages each slide.
- Then `script`, `adventure`, and `journal`.

## M5 — Extract the shared runtime

- Once M1–M3 work here and in Verse, move the common Bevy-side runtime (tool
  executor, clamping, cached replay) into a published crate next to
  `localgpt-world-types` in the LocalGPT workspace, with the model behind a
  trait so Gen can use it too. Verse and MD then depend on crate versions
  instead of copies.

## Open questions

- Free-roam (WASD) as well as the tour, or tour only?
- A WASM build on localgpt.md, or desktop downloads only?
- Export by writing `world.ron` and reusing Gen's HTML/glTF exporters?
