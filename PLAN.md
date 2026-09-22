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

## M1 — Recipe tier: a local LLM per section

- Port Verse's `llm.rs`, `recipe.rs`, and `tier.rs` behind `llm` and
  `llm-metal` features (mistral.rs 0.8; Bonsai-8B Q4_K_M via Verse's
  `scripts/fetch-bonsai.sh`).
- Prompt per section: heading + body + genre → a region recipe (palette,
  landmark kind, props, atmosphere), clamped on the Rust side. Any failure
  keeps the draft.
- Show the draft instantly and swap each region in as its recipe arrives,
  from a background worker like Verse's analysis worker.
- Carry over Verse's constraints (`ARCHITECTURE.md` §10): plain generation
  with a lenient JSON parse, because grammar-constrained generation hangs on
  GGUF; expect about 25 tokens/s on an M2 Max.

## M2 — Section cache (the "lockfile")

- Cache LLM output per section hash in a sidecar next to the document, so an
  unchanged section never regenerates and a shared `.md` plus its sidecar
  renders identically on any machine.
- Fill in `meta.model` and `meta.compliance` once LLM output is included.

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
