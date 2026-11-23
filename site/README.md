# Website content

The page https://menew.pururucos.com.br/linux (and `/pt/linux`, `/de/linux`, `/es/linux`, `/zh/linux`) gets its texts and screenshots from this repository. The site is built and deployed from [Quost/menew-site](https://github.com/Quost/menew-site), whose README describes the content contract in full.

## What lives here

- `site/content.json`: every text of the page, in the 5 languages (`en`, `pt`, `de`, `es`, `zh`), under `"texts"`. All languages must have the same keys; the build fails otherwise. HTML is allowed in some texts (`<code>`, `<b>`, `<span class="grad">`). `"contract": 1` and `"platform": "linux"` identify the format.
- `docs/assets/*.png` (English) and `docs/assets/<lang>/*.png`: the screenshots the page shows (`"assets": "../docs/assets"` in `content.json`). They are the same files the README uses, so there is no second copy to keep. The site publishes them under `/assets/linux/`. A new screenshot only appears on the page if the template in menew-site (`site/templates/linux.html`) references it.

Which keys the page needs is decided by that template. Numbered groups (`pc1_t`/`pc1_d`, `cc…`, `tl…`, `n…`) can grow or shrink: the page shows as many cards as there are.

## What updates by itself

- The version, the **Download** button and the "Preview"/"Stable" label read the newest release of this repository (pre-releases included) from the GitHub API, both when the site is built and when the page loads. A pre-release shows "Preview", a normal release "Stable" (on this page and on the overview). Publish a release and the page follows.
- menew-site checks this repository every 30 minutes and rebuilds the site when `site/` or `docs/assets/` changed on `main`, or a new release is published. No token, secret or workflow is needed here.

## Check and preview

From a checkout of Quost/menew-site next to this repository:

```sh
python3 scripts/content.py linux ../menew-linux/site                    # validate
python3 scripts/build.py --content linux=../menew-linux/site --serve    # preview at http://localhost:8000/linux
```

## Rules

English, Portuguese, German, Spanish and Chinese for every text. No em-dash. Be honest about the state of the port: say what works today, no promises or dates.
