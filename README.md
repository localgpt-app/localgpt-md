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
cargo run -- samples/deck.md         # present a Marp-style deck as a 3D talk
cargo run -- path/to/notes.md        # open any Markdown file
```

The docs are on [localgpt.app/docs/md](https://localgpt.app/docs/md):
[genres and keys](https://localgpt.app/docs/md#genres),
[export](https://localgpt.app/docs/md#export) to the LocalGPT world format,
[screenshot mode](https://localgpt.app/docs/md#screenshot),
[the LLM tier](https://localgpt.app/docs/md/llm) and
[how it works](https://localgpt.app/docs/md/how-it-works).

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Website

[`website/`](website/) is the landing page for
[md.localgpt.app](https://md.localgpt.app/): static HTML, no build step.
`website/deploy.sh` publishes it to Cloudflare as the `localgpt-md` Worker.
The docs live on localgpt.app, in the `localgpt` repository's
`website/docs/md/`.

## License

Apache-2.0. See [LICENSE](LICENSE).
