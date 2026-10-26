# WireTrail brand: where the assets come from

For anyone who updates the identity. This file is not published (`exclude_docs` in `mkdocs.yml`).

The identity is **WireTrail Brand Kit 1.0** (25 October 2026), made for this project and shared in structure with HeapTrail. The kit ships its own guide (`WireTrail-Brand-Guide.html`) and rules (`README.md`, `INTEGRATION.md`). This repository keeps only
what the README, the release notes and the user guide use, not the whole kit.

## What is used, and where it came from

Every file below is byte-identical to the kit, and its SHA-256 matches the kit's `asset-manifest.json`. The prefix is `docs/`.

| In this repository | In the kit | SHA-256 (start) |
|---|---|---|
| `user-guide/assets/brand/wiretrail-mark-on-dark.svg` | `01-logos/marks/wiretrail-mark-on-dark.svg` | `7baceb52580d` |
| `user-guide/assets/brand/favicon.svg` | `02-platform-icons/web/favicon.svg` | `aba601bb0909` |
| `user-guide/assets/brand/favicon.ico` | `02-platform-icons/web/favicon.ico` | `55d43ce9c177` |
| `user-guide/assets/brand/favicon-32x32.png` | `02-platform-icons/web/favicon-32x32.png` | `524ef1d56f3c` |
| `user-guide/assets/brand/favicon-16x16.png` | `02-platform-icons/web/favicon-16x16.png` | `585f1a545de3` |
| `user-guide/assets/brand/apple-touch-icon.png` | `02-platform-icons/web/apple-touch-icon.png` | `4c9d46861ffe` |
| `user-guide/assets/brand/wiretrail-social-dark-1200x630.png` | `03-banners/wiretrail-social-dark-1200x630.png` | `433aa24c18e1` |
| `assets/brand/wiretrail-readme-light-1600x480.png` | `03-banners/wiretrail-readme-light-1600x480.png` | `062fe83c7e9c` |
| `assets/brand/wiretrail-readme-dark-1600x480.png` | `03-banners/wiretrail-readme-dark-1600x480.png` | `9ae9d4c70d2e` |
| `assets/brand/wiretrail-github-dark-1280x640.png` | `03-banners/wiretrail-github-dark-1280x640.png` | `4b82511dbc5a` |
| `user-guide/assets/fonts/SpaceGrotesk.woff` | `07-fonts/SpaceGrotesk.woff` | `7b7f926bd3f5` |
| `user-guide/assets/fonts/Inter.woff` | `07-fonts/Inter.woff` | `dcfc3feb79a1` |
| `user-guide/assets/fonts/IBMPlexMono-Regular.woff` | `07-fonts/IBMPlexMono-Regular.woff` | `b4719702a5c6` |
| `user-guide/assets/fonts/*-OFL.txt` (three files) | `07-fonts/*-OFL.txt` | see the kit |

The three typefaces are byte-identical to the ones in HeapTrail's kit, because the two kits share typography.

Two copies are changed on purpose:

- **The 16 glyphs** in `overrides/.icons/wiretrail/` come from `04-integration-artwork/ui-icons/`. The kit labels each with `role="img"`, `aria-label` and a `<title>`. They sit beside a visible heading here, so those are replaced with `aria-hidden="true"`,
  as the kit's `INTEGRATION.md` advises for decorative glyphs. The paths are untouched.
- **The color tokens** in `user-guide/stylesheets/wiretrail.css` are the values of `06-implementation/brand-tokens.css`, written under Material's `data-md-color-scheme` selectors (`default` and `slate`) instead of the kit's `data-theme`, and prefixed
  `--wt-`. The code-highlighting colors are chosen from the same tokens. The stylesheet was derived from HeapTrail's by changing only the prefix and the colors.

## Where each thing is used

| Asset | Used by |
|---|---|
| `wiretrail-readme-{light,dark}-1600x480.png` | `README.md`, in a `<picture>` that follows the reader's color scheme, and the top of each release's notes (see `release-notes.md`). The README uses full `https://github.com/.../raw/HEAD/...` addresses for both banners, not relative paths: crates.io rewrites the `<img>` address but leaves the `<source srcset>` relative, so a relative dark banner is a broken image for dark-mode visitors on the crate page. A published crate's README cannot be edited, so a mistake here costs a release |
| `wiretrail-github-dark-1280x640.png` | GitHub's social preview. GitHub has no API for it: upload the file in Settings, General, Social preview |
| `wiretrail-social-dark-1200x630.png` | `og:image` and `twitter:image` on every page, from `overrides/main.html` |
| `wiretrail-mark-on-dark.svg` | The header logo. The header is Night in both color schemes, so the on-dark mark is always right |
| `favicon.svg`, `favicon.ico`, `apple-touch-icon.png` | The browser tab and the home-screen icon. `favicon-16x16.png` and `favicon-32x32.png` are kept for tools that look for them |
| The three `.woff` fonts | `@font-face` in `wiretrail.css`. `font: false` in `mkdocs.yml` keeps Material from loading Google Fonts |
| The 16 glyphs | `:wiretrail-network:` and so on, through `pymdownx.emoji` in `mkdocs.yml`. The home page cards use nine of them |

Not used, on purpose: the PWA manifest and the maskable icons (a documentation site is not an installable app), the platform icon sets for Windows, macOS and Linux (WireTrail is a command-line tool with no GUI), the template and monochrome artwork,
and the background pattern. They stay in the kit.

## Rules from the kit that the guide follows

- Write WireTrail in prose and branding. The executable is `wiretrail`. `scripts/docs-check.py` enforces this in the pages and in the README.
- Clear space of 16 units around a 100-unit mark. The horizontal logo is at least 160 px wide; below that, use the mark. Do not distort, rotate or add effects.
- Light surfaces use `#006779` for links and filled controls, with white text. Dark surfaces use `#28C5DC` for filled controls, with Night text, and `#6FDEEC` for links.
- Status colors always come with a label, an icon or a code. The product color is never a severity scale.
- Keep machine output stable: no logo, color code or marketing in JSON, and respect `NO_COLOR`. Branding belongs in documentation, release material and package listings, not in the terminal output.
- WireTrail is a CLI. The kit's platform icons are not a claim that a GUI exists.

## Licence and trademark

The kit's README says that the MIT licence of the WireTrail code "is not a new declaration about this artwork", that the artwork was created for the kit, and that no trademark clearance is claimed. This repository adds no licence statement for the artwork either.
The fonts keep their SIL Open Font License notices, which travel with them in `assets/fonts/`.

## Updating to a new kit

1. Unpack the new kit into an empty folder, outside the repository.
2. Compare the files in the table above with `shasum -a 256` and the kit's `asset-manifest.json`. Replace the ones that changed.
3. Copy the glyphs again and replace `role`, `aria-label` and `<title>` with `aria-hidden="true"`.
4. Compare `06-implementation/brand-tokens.css` with the token blocks in `wiretrail.css`.
5. Update the table above, then run the checks in `style.md`.
