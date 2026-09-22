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

Serve this directory from any static host (GitHub Pages, Cloudflare Pages,
Netlify). There is nothing to compile; upload `website/` as-is.

The page links back to [localgpt.app](https://localgpt.app), the main
LocalGPT site, and should keep doing so.
