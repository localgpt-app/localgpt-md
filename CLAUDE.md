# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

LocalGPT MD opens a Markdown file and turns it into a walkable 3D world
(Bevy 0.19). Each `##` section becomes a place; the file is watched and the
world rebuilds on save. Standalone Cargo project (its own `[workspace]`),
Apache-2.0. Siblings: LocalGPT Verse (song → world) and LocalGPT Gen
(prompt → world).

## Commands

```bash
cargo run                                        # open samples/hello.md
cargo run -- path/to/doc.md                      # open any Markdown file
cargo run -- doc.md --print-ron                  # compiled world as RON, no window
LOCALGPT_MD_SCREENSHOT=/tmp/shot.png cargo run   # render the first stop offscreen to a PNG, then exit
cargo test
cargo clippy --all-targets -- -D warnings
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
(`Doc` → `WorldManifest`) → `scene.rs` (manifest → Bevy entities), plus
`tour.rs` (camera and caption) and `watch.rs` (polling hot reload).

- **`doc.rs` and `draft.rs` are pure**: no Bevy imports. Keep it that way;
  they are what moves into a shared crate later (PLAN.md M5).
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
  edits, which the per-section LLM cache (M2) relies on.
- **`Section::hash` is the cache key** for LLM output. Anything that should
  trigger regeneration must be part of the hashed text.

## Bevy 0.19 notes

Match LocalGPT Verse's idioms (`../localgpt-verse/src`): `Hdr` is its own
component (`bevy::camera::Hdr`); scene-wide ambient light is the
`GlobalAmbientLight` resource; `TextFont { font_size: FontSize::Px(..), .. }`;
buffered events are messages (`MessageWriter<AppExit>`); hierarchy is
`ChildOf(parent)`, and `despawn()` is recursive.

## Plan and reuse

See PLAN.md. The LLM tiers get ported from LocalGPT Verse (`src/llm.rs`,
`src/recipe.rs`, `src/agent.rs`, `src/agent_types.rs`, `src/tier.rs`) behind
`llm` / `llm-metal` features. Read Verse's `ARCHITECTURE.md` §10 first: it
records the mistral.rs 0.8 constraints (grammar-constrained generation hangs
on GGUF; the 8B Q4_K_M model needs Metal). Verse and LocalGPT are Apache-2.0,
so copying from them is fine; name the source in a comment.

## Rules

- License is Apache-2.0. Never copy code from Local Native or Fastxt (AGPL-3.0).
- This repo is public. Never name the closed-source sibling 3D platform; use
  generic terms such as "connected 3D app".
- Commits: conventional commits (`feat:`, `fix:`, `docs:`, `chore:`,
  `refactor:`), with no Co-Authored-By lines.
- Never use `sed` to edit Rust files; use the Edit tool.
- `website/` is the static landing page for localgpt.md (no build step). It
  links back to localgpt.app.
