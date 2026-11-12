# Amerika'nın Tarihi Yok Ki(!?) — Nota Vault & Static Site

This repository hosts the Markdown vault, spaced-repetition review logs, and GitHub Pages deployment configuration for **Amerika'nın Tarihi Yok Ki(!?)**, powered by **Nota** (`Mustafa-Dara-Ozevin/nota`).

## Folder Structure

```text
.
├── .github/
│   └── workflows/
│       └── deploy.yml      # GitHub Actions workflow that builds & deploys the site with Nota
├── content/                # Markdown notes, wikilinks, logic blocks, and media assets
│   └── assets/             # Embedded images and attachments (e.g. ![[image.png]])
├── reviews/                # FSRS spaced-repetition logs (revlog.jsonl, schedule.json)
├── nota.toml               # Nota vault, symbolic logic, SRS, and site configuration
├── typing-rules.conf       # Typing transformer rules for symbolic logic operators
├── public/                 # Local Nota static site output (gitignored)
└── public-nota/            # Alternate local Nota build output (gitignored)
```

## Nota Site Generation

Nota (`nota-cli` / `nota-site`) compiles the Markdown vault in `content/` into a static HTML site without Node.js or Quartz:

- **Markdown & Wikilinks**: Converts `[[Target|Alias]]` wikilinks, `![[image.png]]` embeds, callouts, tables, and frontmatter aliases into clean static HTML and alias redirects.
- **Symbolic Logic & Truth Trees**: Renders ` ```tree ` / ` ```tableau ` blocks and truth tables into deterministic HTML/SVG without math-delimiter corruption.
- **Navigation & Feeds**: Generates tag index pages, search index data, RSS (`index.xml`), and `sitemap.xml` using the `[site]` settings in `nota.toml` (`base_url = "mustafa-dara-ozevin.github.io/American_Studies"`).

### Local Preview & Editing

Open the vault in the Nota desktop app:

```bash
nota open --vault content
```

Build the static site locally and preview it in a browser:

```bash
nota build --vault content --out public-nota --base-url http://localhost:8080
python3 -m http.server 8080 --directory public-nota
```

Then open `http://localhost:8080/` in your browser.

### Automated GitHub Pages Deployment

Pushing to `v4` (or `main`) triggers `.github/workflows/deploy.yml`:

1. Checks out this vault repository.
2. Checks out the private `Mustafa-Dara-Ozevin/nota` repository using the `NOTA_REPO_TOKEN` repository secret.
3. Builds `nota-cli` in release mode (cached via `actions/cache`).
4. Runs `nota build --vault content --out public`.
5. Uploads `public/` and deploys it to GitHub Pages.
