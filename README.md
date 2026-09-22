# LocalGPT MD

Open a Markdown file and walk through it as a 3D world. Every `##` section
becomes a place, and saving the file rebuilds the world while you watch.

Built with [Bevy](https://bevyengine.org/). Part of [LocalGPT](https://localgpt.app).

**Status: M2.** Worlds start from a fast rule-based draft, then a local LLM
(feature `llm`) restyles each region from what the prose actually says —
palette, landmark, props — and caches the result per section in a sidecar
next to the document, so nothing is generated twice. The on-device model and
inference path are ported from LocalGPT Verse (Bonsai-8B via mistral.rs; see
[PLAN.md](PLAN.md)).

## Run

```bash
cargo run                            # open samples/hello.md
cargo run -- path/to/notes.md        # open any Markdown file
cargo run -- notes.md --print-ron    # print the compiled world as RON and exit
```

Keep the file open in your editor: every save rebuilds the world, and the
camera stays on the section you're looking at.

| Key | Action |
|---|---|
| → ↓ Space PageDown | Next section |
| ← ↑ PageUp | Previous section |
| Home / End | First / last section |

`LOCALGPT_MD_SCREENSHOT=out.png cargo run` renders the first stop to a PNG
and exits, for a quick check without a human. It renders offscreen with no
window, so it also works over SSH or while the screen is locked.

## The LLM tier (optional)

```bash
./scripts/fetch-bonsai.sh                              # ~5.2 GB model, once
cargo run --features llm-metal --                      # styled live, in-app
cargo run --features llm-metal -- notes.md --generate  # headless: style all, exit
```

With the feature and a model present, a background worker styles every
section that has no cached recipe, and each region upgrades in place as its
recipe arrives. Results live in `notes.md` → `notes.world.json` — a lockfile
keyed by each section's content hash: unchanged sections are never
regenerated, and a `.md` plus its sidecar renders identically on any machine,
even in the default (no-`llm`) build, which still applies cached recipes.
Every model-authored value is clamped on the Rust side, and any generation
failure keeps the rule-based draft — the app is never broken by a missing or
misbehaving model. `llm-metal` is the macOS GPU path (the 5 GB Q4_K_M needs
it to fit in memory); `llm` alone expects a smaller GGUF. Any standard GGUF
plus a matching `tokenizer.json` dropped in `assets/llm/` (or
`$LOCALGPT_MD_LLM`) is picked up — the sibling Verse checkout's model
directory is searched too.

## How it works

```
FILE.md ──► doc.rs ──────► draft.rs ─────────► scene.rs ──► Bevy
            sections +     WorldManifest       entities,
            BLAKE3 hashes  (localgpt-world-    lights, fog
                            types)                  ▲
   ▲                                                │
watch.rs: poll the file, recompile on save     tour.rs: camera + caption
```

- **`doc.rs`**: Markdown → title, front matter (`genre: …`), and one section
  per `##` heading, each with a hash of its text.
- **`draft.rs`**: sections → a `WorldManifest` with one region per section
  along a winding path, and a tour with one stop per section. Deterministic,
  with entity ids that are stable per section, so editing one section leaves
  the other regions untouched. A cached recipe for a section's hash restyles
  just that region.
- **`recipe.rs` / `sidecar.rs`** (pure): the clamped per-region recipe type
  with its lenient LLM-reply parse, and the hash-keyed `<doc>.world.json`
  cache. Compiled in every build; only the authoring is feature-gated.
- **`llm.rs` / `generation.rs`** (`llm` feature): the model (ported from
  Verse) and the worker thread that styles uncached sections in the
  background while the draft is already on screen.
- **`scene.rs`**: spawns the manifest in Bevy using the same mapping as
  LocalGPT Gen.
- **`tour.rs`** and **`watch.rs`**: navigation and hot reload.

Worlds use [`localgpt-world-types`](https://crates.io/crates/localgpt-world-types),
the format LocalGPT Gen saves as `world.ron`, and pass Gen's own validation.
`--print-ron` prints exactly that file.

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Website

[`website/`](website/) is the landing page for localgpt.md: static HTML, no
build step. `./scripts/deploy.sh` publishes it to Cloudflare as the
`localgpt-md` Worker.

## License

Apache-2.0. See [LICENSE](LICENSE).
