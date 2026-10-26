# WireTrail user guide

The user guide is a site: **<https://wiretrail.neerdael.nl/>**. This file used to hold it as one long document, with examples from a private capture. The guide now has tutorials, how-tos, recipes, concepts and a
reference, and every block of output in it comes from a real run on demo captures that you can download.

Each section of the old document has a home in the guide:

| Old section | Now |
|---|---|
| 1. Capturing a HAR | [Capture](https://wiretrail.neerdael.nl/capture/) |
| 2. Cheat sheet | [Choose the right command](https://wiretrail.neerdael.nl/getting-started/choose-a-command/) and [Command line](https://wiretrail.neerdael.nl/reference/command-line/) |
| 3. `summary` | [Summary](https://wiretrail.neerdael.nl/analyze/summary/) |
| 4. Wasteful traffic | [Repeats](https://wiretrail.neerdael.nl/repeats/): [duplicates](https://wiretrail.neerdael.nl/repeats/duplicates/), [retries](https://wiretrail.neerdael.nl/repeats/retries/), [storms](https://wiretrail.neerdael.nl/repeats/storms/), [pagination](https://wiretrail.neerdael.nl/repeats/pagination/), [rate limits](https://wiretrail.neerdael.nl/repeats/rate-limits/) |
| 5. Failures and timing | [Failures](https://wiretrail.neerdael.nl/failures/): [errors](https://wiretrail.neerdael.nl/failures/errors/), [slowest calls](https://wiretrail.neerdael.nl/failures/slowest/), [status transitions](https://wiretrail.neerdael.nl/failures/transitions/) |
| 6. Auth | [Auth failures and refresh](https://wiretrail.neerdael.nl/failures/auth/), [JWTs](https://wiretrail.neerdael.nl/failures/jwt/), [Handoff](https://wiretrail.neerdael.nl/inspect/handoff/) |
| 7. Inspection, diff and export | [Inspect](https://wiretrail.neerdael.nl/inspect/) and [What varies between calls](https://wiretrail.neerdael.nl/repeats/diff/) |
| 8. Diagnosis and capture quality | [Auto](https://wiretrail.neerdael.nl/analyze/auto/), [Diagnose](https://wiretrail.neerdael.nl/analyze/diagnose/), [Validate](https://wiretrail.neerdael.nl/analyze/validate/), [Startup](https://wiretrail.neerdael.nl/failures/startup/), [Cascade](https://wiretrail.neerdael.nl/failures/cascade/) |
| 9. Regression and rules | [Compare](https://wiretrail.neerdael.nl/compare/) |
| 10. `--json` | [JSON output](https://wiretrail.neerdael.nl/automate/json-output/) and [JSON schemas](https://wiretrail.neerdael.nl/reference/json-schemas/) |
| 11. End to end | [Your first triage](https://wiretrail.neerdael.nl/getting-started/first-triage/) and the recipes in each section |
| 12. Performance and limits | [Performance](https://wiretrail.neerdael.nl/reference/performance/) and [Limitations](https://wiretrail.neerdael.nl/reference/limitations/) |
| 13. Troubleshooting | [Troubleshooting](https://wiretrail.neerdael.nl/help/troubleshooting/) |

The guide also explains how [redaction works](https://wiretrail.neerdael.nl/concepts/redaction/), including what it does not mask. The old text said every command redacts by default. Most do, with exceptions that matter.

The sources of the guide are in [`docs/user-guide/`](docs/user-guide/).
