# Download and install

WireTrail is one executable with no runtime dependencies. Pick whichever way suits you.

## Download a prebuilt binary

Each [release](https://github.com/johnneerdael/wiretrail/releases/latest) has six archives. Each holds
a single file, `wiretrail` (or `wiretrail.exe` on Windows).

| Platform | Archive |
|---|---|
| macOS, Apple silicon | `wiretrail-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `wiretrail-x86_64-apple-darwin.tar.gz` |
| Linux, x86-64 | `wiretrail-x86_64-unknown-linux-gnu.tar.gz` |
| Linux, ARM64 | `wiretrail-aarch64-unknown-linux-gnu.tar.gz` |
| Windows, x86-64 | `wiretrail-x86_64-pc-windows-msvc.zip` |
| Windows, ARM64 | `wiretrail-aarch64-pc-windows-msvc.zip` |

On macOS or Linux, download, unpack and put the file on your `PATH`:

```bash
curl -fsSLO https://github.com/johnneerdael/wiretrail/releases/latest/download/wiretrail-aarch64-apple-darwin.tar.gz
tar -xzf wiretrail-aarch64-apple-darwin.tar.gz
sudo mv wiretrail /usr/local/bin/
```

Use the archive name from the table that matches your machine. On Windows, unzip the archive and put
`wiretrail.exe` in a folder that is on your `PATH`.

!!! note "The binaries are not code-signed"
    The release workflow builds them from the tagged commit, and they carry no signature. A binary
    downloaded in a browser can be blocked by macOS Gatekeeper or warned about by Windows SmartScreen.
    Downloading with `curl`, as above, does not set the flag that triggers the macOS block. If your
    system still blocks the file, build from source instead.

## Install with cargo

If you have a Rust toolchain (version 1.88 or newer), install the published crate:

```bash
cargo install wiretrail
```

To update later, run `cargo install wiretrail --force`.

## Build from source

```bash
git clone https://github.com/johnneerdael/wiretrail
cd wiretrail
cargo build --release
```

The executable is `target/release/wiretrail`. Copy it onto your `PATH`, or run it from there.

## Check that it works

```bash
wiretrail --version
```

<!-- output: version.txt -->
```text
wiretrail 0.3.2
```

If you see a version, you are ready. If the shell says `command not found`, the folder that holds
`wiretrail` is not on your `PATH`. See [Troubleshooting](../help/troubleshooting.md).

To see every command, run `wiretrail --help`. [Command line](../reference/command-line.md) explains each one.

## Uninstall

Delete the `wiretrail` file. If you installed with cargo, run `cargo uninstall wiretrail`.

## Related

- [Your first triage](first-triage.md) runs the first command on a real capture.
- [Follow along with the demo captures](follow-along.md) has files you can try it on right away.
- [Handle captures safely](../capture/handle-captures-safely.md) before you analyze a capture from production.
