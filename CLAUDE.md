# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

LocalGPT MD opens a Markdown file and turns it into a walkable 3D world
(Bevy 0.19). Each `##` section becomes a place; the file is watched and the
world rebuilds on save. An optional local LLM (`llm` feature, ported from
LocalGPT Verse) restyles regions from the prose and caches per section.
Standalone Cargo project (its own `[workspace]`), Apache-2.0. Siblings:
LocalGPT Verse (song → world) and LocalGPT Gen (prompt → world).

## Commands

```bash
cargo run                                        # open samples/hello.md (world genre)
cargo run -- samples/deck.md                     # present a Marp-style deck (deck genre)
cargo run -- path/to/doc.md                      # open any Markdown file
cargo run -- doc.md --print-ron                  # compiled world as RON, no window
LOCALGPT_MD_SCREENSHOT=/tmp/shot.png cargo run   # render the first stop offscreen to a PNG, then exit
./scripts/fetch-bonsai.sh                        # fetch the ~5.2 GB LLM (once)
cargo run --features llm-metal --                # open with live LLM styling
cargo run --features llm-metal -- doc.md --generate   # style all sections headless, exit
cargo test
cargo clippy --all-targets -- -D warnings        # and again with --features llm-metal
cargo fmt
```

Run `cargo check` after every change and fix all errors before reporting
completion. Run clippy and fmt before committing.

To check rendering, use the screenshot mode and read the PNG. It renders to
an offscreen image with no window, because reading back a window's frame
returns solid black when the Mac is locked or headless (Verse hit the same
thing; see its ARCHITECTURE.md R8). Without a window, the camera has to be
marked `IsDefaultUiCamera`, or the UI isn't drawn.

## Architecture

`doc.rs` (Markdown → `Doc` of sections with BLAKE3 hashes) → `draft.rs`
(`Doc` + recipes → `WorldManifest`) → `scene.rs` (manifest → Bevy entities),
plus `tour.rs` (camera and caption), `watch.rs` (polling hot reload),
`recipe.rs`/`sidecar.rs` (recipe type + cache), and `llm.rs`/`generation.rs`
(the `llm` feature: model + background worker).

- **`doc.rs`, `draft.rs`, `recipe.rs`, `sidecar.rs` are pure** (no Bevy
  rendering; sidecar's only Bevy touch is the `Resource` derive). Keep them
  that way; they are what moves into a shared crate later (PLAN.md M5).
- **Genres** (`front_matter.genre`): `world` (default) — one section per
  `##` heading on a winding path; `deck` — one section per `---`-separated
  slide (Marp/Slidev) on a straight path. Deck separators must be
  blank-line padded: a bare `---` directly under text is a CommonMark setext
  H2, not a separator. In a deck, headings don't start sections; the first
  heading in a slide names it.
- **The world format is `localgpt-world-types`** (crates.io, serde-only).
  Don't invent a parallel scene format; if something is missing, add it
  upstream in `localgpt/crates/world-types`. Every manifest must pass
  `draft::validate` (Gen's save-time checks); a test in `draft.rs` enforces it.
- **`scene.rs` mirrors Gen's mapping** (`localgpt/crates/gen/src/gen3d/plugin.rs`):
  sRGB colours, linear emissive, `EulerRot::XYZ` in degrees, lux for
  directional lights and lumens for point/spot, spot angles in radians. Keep
  them in sync so a manifest renders the same in both apps.
- **Entity ids are stable per section**: `(section_index + 1) * 1000 + n`;
  ids 1–999 are global (ground, sun). Unchanged sections stay identical across
  edits, which the per-section cache relies on.
- **`Section::hash` is the cache key** for LLM output. Anything that should
  trigger regeneration must be part of the hashed text.
- **Recipes degrade, never break**: every `RegionRecipe` field is optional,
  everything is clamped (`RegionRecipe::clamped`, again on sidecar insert and
  load), and a failed generation keeps the draft. The default (no-`llm`)
  build still *applies* cached sidecars — which is why recipe/sidecar are
  unconditionally compiled (targeted `#[allow(dead_code)]`, Verse `tier.rs`
  precedent).
- **The LLM constraints are Verse's, verified** (`../localgpt-verse/ARCHITECTURE.md`
  §10): plain instructed-JSON generation with a lenient parse — mistral.rs
  0.8's grammar-constrained `generate_structured` hangs on GGUF (so no
  `schemars`); the 5 GB Q4_K_M needs `llm-metal` (macOS-only, never in the
  Linux CI job); model discovery is `$LOCALGPT_MD_LLM` → `assets/llm` →
  Verse's `assets/llm`.

## Bevy 0.19 notes

Match LocalGPT Verse's idioms (`../localgpt-verse/src`): `Hdr` is its own
component (`bevy::camera::Hdr`); scene-wide ambient light is the
`GlobalAmbientLight` resource; `TextFont { font_size: FontSize::Px(..), .. }`;
buffered events are messages (`MessageWriter<AppExit>`); hierarchy is
`ChildOf(parent)`, and `despawn()` is recursive.

## Plan and reuse

See PLAN.md. M0–M2 are done (draft, LLM recipe tier, sidecar cache). Next:
M3 (agent tier — Verse's `agent.rs`/`agent_types.rs` tool-calling port),
M4 (genres, `deck` first), M5 (extract the shared runtime). Verse and
LocalGPT are Apache-2.0, so copying from them is fine; name the source in a
comment. Model notes: Bonsai-8B Q4_K_M is the verified default; stock
Qwen3-8B-Instruct Q4_K_M is a drop-in A/B; the newer ternary Bonsai
generations (1-bit/2-bit) cannot run on mistral.rs 0.8 — skip them.

## Rules

- License is Apache-2.0. Never copy code from Local Native or Fastxt (AGPL-3.0).
- This repo is public. Never name the closed-source sibling 3D platform; use
  generic terms such as "connected 3D app".
- Commits: conventional commits (`feat:`, `fix:`, `docs:`, `chore:`,
  `refactor:`), with no Co-Authored-By lines.
- Never use `sed` to edit Rust files; use the Edit tool.
- `website/` is the static landing page for localgpt.md (no build step). It
  links back to localgpt.app.
