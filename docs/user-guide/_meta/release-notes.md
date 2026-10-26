# WireTrail release notes: format

For whoever cuts a release. This file is not published (`exclude_docs` in `mkdocs.yml`).

A release has two sets of notes, and they say the same thing:

- **The GitHub release**, which GitHub sends to everyone watching the repository. It carries the banner, the highlights, the downloads and the checks that were run.
- **The What's new page** (`getting-started/whats-new.md`), which keeps every release in one place with the guide's voice and links to the pages that changed.

## The GitHub release

Title: `WireTrail vX.Y.Z`. Write WireTrail in the title and the body, as the brand guide says; the executable stays `wiretrail`.

The body, in this order:

````markdown
![WireTrail. Follow the request. Find the cause.](https://raw.githubusercontent.com/johnneerdael/wiretrail/vX.Y.Z/docs/assets/brand/wiretrail-readme-dark-1600x480.png)

WireTrail — Follow the request. Find the cause.

## What's new

- **One bold sentence that says what changed for the reader.** Then what it means and what to do about it, in plain words.
- **Next change.** Say what the tool does now, not what the code does.

See [the user guide](https://wiretrail.neerdael.nl/) for the pages that changed.

## Downloads

| Download | Platform |
|---|---|
| [wiretrail-aarch64-apple-darwin.tar.gz](https://github.com/johnneerdael/wiretrail/releases/download/vX.Y.Z/wiretrail-aarch64-apple-darwin.tar.gz) | macOS, Apple silicon |
| [wiretrail-x86_64-apple-darwin.tar.gz](https://github.com/johnneerdael/wiretrail/releases/download/vX.Y.Z/wiretrail-x86_64-apple-darwin.tar.gz) | macOS, Intel |
| [wiretrail-x86_64-unknown-linux-gnu.tar.gz](https://github.com/johnneerdael/wiretrail/releases/download/vX.Y.Z/wiretrail-x86_64-unknown-linux-gnu.tar.gz) | Linux, x86-64 |
| [wiretrail-aarch64-unknown-linux-gnu.tar.gz](https://github.com/johnneerdael/wiretrail/releases/download/vX.Y.Z/wiretrail-aarch64-unknown-linux-gnu.tar.gz) | Linux, ARM64 |
| [wiretrail-x86_64-pc-windows-msvc.zip](https://github.com/johnneerdael/wiretrail/releases/download/vX.Y.Z/wiretrail-x86_64-pc-windows-msvc.zip) | Windows, x86-64 |
| [wiretrail-aarch64-pc-windows-msvc.zip](https://github.com/johnneerdael/wiretrail/releases/download/vX.Y.Z/wiretrail-aarch64-pc-windows-msvc.zip) | Windows, ARM64 |

Or install from source with `cargo install wiretrail`. The binaries are built by the release workflow from the tagged commit and are not code-signed.

Validation:
- cargo fmt --check
- cargo clippy --workspace --all-targets --all-features -- -D warnings
- cargo test (N tests)
- anything else that was run, and what it showed
````

Rules for the body:

- **Pin the banner to the tag** (`.../vX.Y.Z/...`), so the image survives a later rename or move. The tag must contain `docs/assets/brand/`.
- **Lead with the reader's gain.** "`diagnose` now names the host of every finding" is a bold lead. "Refactor ranking" is not.
- **State the evidence.** Name the command or the page, give the number you measured, and say on which machine. Do not write "faster" without one.
- **Keep claims grounded.** Say what a change does not do. A finding is a pattern, and not a diagnosis.
- **Do not put the banner, a color code or marketing in the JSON or the terminal output.** The brand lives in the notes, the guide and the package listing.

## Making the release

After the pull request is merged and the guide has deployed:

```bash
gh release create vX.Y.Z --target main \
  --title "WireTrail vX.Y.Z" --notes-file notes.md --latest
```

Creating the release starts `release.yml`, which builds the six binaries and publishes the crate with the repository's `CARGO_REGISTRY_TOKEN`. The banner needs the tag to contain `docs/assets/brand/`, which it does from the commit that introduced the brand assets onward.
Creating the release is the step that cannot be undone, and a published crate's README cannot be edited.

## The What's new page

Add a section above the previous one, with the date the release is published:

```markdown
## X.Y.Z — Month D, YYYY

One sentence that says what the release is about.

- What changed, in the guide's voice, with a link to the page.
```

Add a row to the "Which version added what" table only when the release adds something a reader would need a version for. Add the same entry to `CHANGELOG.md`.
