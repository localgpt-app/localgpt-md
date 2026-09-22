# LocalGPT MD

Open a Markdown file and walk through it as a 3D world. Every `##` section
becomes a place, and saving the file rebuilds the world while you watch.

Built with [Bevy](https://bevyengine.org/). Part of [LocalGPT](https://localgpt.app).

**Status: scaffold (M0).** Worlds come from a fast rule-based draft: layout,
size, colour, and shape are derived from each section's text and content
hash. The on-device LLM that actually *reads* the prose — ported from
LocalGPT Verse — is the next milestone. See [PLAN.md](PLAN.md).

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
  the other regions untouched.
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
build step.

## License

Apache-2.0. See [LICENSE](LICENSE).
