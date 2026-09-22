# LocalGPT MD website

The landing page for localgpt.md. Hand-written static HTML and CSS: no build
step, no dependencies, and no external requests.

## View locally

```bash
cd website && python3 -m http.server 8000
# open http://localhost:8000
```

Opening `index.html` directly from disk also works.

## Deploy

```bash
../scripts/deploy.sh
```

Deploys this directory as the `localgpt-md` Cloudflare Worker (static
assets), served at `https://localgpt-md.<account-subdomain>.workers.dev`.
Log in once with `npx wrangler login` first; the script will remind you if
you haven't. There is nothing to compile; `website/` is uploaded as-is,
except the files listed in `.assetsignore`.

Any other static host (GitHub Pages, Netlify) also works — serve the
directory directly.

The header links to the sibling app [verse.localgpt.app](https://verse.localgpt.app).
The page links back to [localgpt.app](https://localgpt.app), the main
LocalGPT site, and should keep doing so.
