# Website content

The page https://menew.web.app/linux (and `/pt/linux`, `/de/linux`, `/es/linux`, `/zh/linux`) is controlled from this repository.

## What lives here

- `site/linux.json`: every text of the page, in the 5 languages (`en`, `pt`, `de`, `es`, `zh`). All languages must have the same keys; the build fails otherwise. HTML is allowed in some texts (`<code>`, `<b>`, `<span class="grad">`).
- `docs/assets/*.png` (English) and `docs/assets/<lang>/*.png`: the screenshots the page shows. They are the same files the README uses, so there is no second copy to keep. A new screenshot only appears on the page if the template in the Mac repo (`site/linux.html`) references it.

The layout and the look (`site/linux.html`) stay in the Mac repo, because `menew.web.app` is one Firebase site and the design is shared with the rest of it.

## What updates by itself

- The version and the **Download** button in the hero read the newest release of this repository (pre-releases included) from the GitHub API when the page loads. They need nothing from you: publish a release and the page shows it. Without JavaScript, or if the API fails, the page shows the version in the root `Cargo.toml` as of the last build.
- The rest (texts and screenshots) is built at deploy time in `Quost/menew`, which clones this repository at `main`. This repository's `Site sync` workflow (`.github/workflows/site-sync.yml`) asks that repo to rebuild when a release is published and when `main` changes `site/**` or `docs/assets/**`. It needs the secret `MENEW_SITE_DISPATCH_TOKEN` (a fine-grained token with write access to `Quost/menew`); without it the workflow only prints a notice and the site picks the content up on its next rebuild.

## Preview locally

In a checkout of the Mac repo (`Quost/menew`), with this repository next to it:

```sh
MENEW_LINUX_DIR=../menew-linux python3 scripts/build-site.py
python3 -m http.server -d docs 8000   # then open http://localhost:8000/linux
```

The generated Linux pages are not committed in the Mac repo (they are in its `.gitignore`).

## Rules

English, Portuguese, German, Spanish and Chinese for every text. No em-dash. Be honest about the state of the port: say what works today, no promises or dates.
