# Device-driver Pregen Check

A GitHub Action that fails your build when the Rust file you committed no longer
matches your [device-driver](https://device-driver.com/) v2 DDSL source.

It regenerates the driver into a temporary file, formats it, and diffs it against
your committed file. Nothing is ever rewritten or committed for you.

## Quick start

Add `.github/workflows/pregen.yml` to your driver repository:

```yaml
name: Device-driver pregen check

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

concurrency:
  group: ${{ github.workflow }}-${{ github.head_ref || github.run_id }}
  cancel-in-progress: true

jobs:
  pregen:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v6
        with:
          persist-credentials: false
      - uses: tullom/device-driver-pregen-check@v1
        with:
          source: device.ddsl
          generated-file: src/device.rs
```

Point `source` at your DDSL file and `generated-file` at the Rust file you
commit. Everything else has a default. For immutable pinning, replace `@v1` with
a full commit SHA.

To generate `defmt` implementations gated by a Cargo feature, set
`rust-defmt-feature` to that feature's name:

```yaml
      - uses: tullom/device-driver-pregen-check@v1
        with:
          source: device.ddsl
          generated-file: src/device.rs
          rust-defmt-feature: defmt
```

Omit it or leave it empty to generate without `defmt` implementations.

## How it works

1. Installs Rust with `rustfmt`, then `device-driver-cli` (cached between runs).
2. Runs `ddc build` into a temporary file at your repository root.
3. Formats it with `rustfmt --edition 2024 --config newline_style=Unix`.
4. Diffs it against `generated-file`.

The temporary file is always cleaned up. There are no outputs; the step simply
passes or fails.

## Inputs

| Input | Required | Default | Description |
| --- | --- | --- | --- |
| `source` | yes | — | Your DDSL file, relative to the repository root |
| `generated-file` | no | `src/device.rs` | The committed Rust file to check |
| `cli-version` | no | `2.1.0` | Oldest `device-driver-cli` v2 release you accept |
| `rust-toolchain` | no | `1.94.0` | Exact Rust release to use |
| `rust-defmt-feature` | no | `''` | Cargo feature gating generated `defmt` implementations; empty disables them |

**Paths** are relative to your repository root and use forward slashes. Spaces
are fine. Absolute paths, backslashes, directories, and anything outside the
checkout are rejected.

**`cli-version` is a floor, not a pin.** The default installs the newest
[`device-driver-cli`](https://crates.io/crates/device-driver-cli) in
`>=2.1.0, <3.0.0`. Ranges, prereleases, and `latest` are rejected, so raise the
floor with an exact release like `2.3.1`. `rust-toolchain` must be an exact
stable release, at least 1.94.0 and new enough for the CLI.

**Formatting** uses the `rustfmt.toml` at your repository root; config files in
other directories are ignored. Edition 2024 and LF newlines are always forced.

The diff is exact, so line endings matter. Add this to your `.gitattributes`:

```gitattributes
*.rs text eol=lf
```

## Requirements

- An Ubuntu GitHub-hosted runner.
- A checkout step first; the action doesn't check out anything itself.
- `permissions: contents: read`. No secrets.
- `submodules: true` on your checkout if the DDSL lives in a submodule.

If your organization restricts actions, allow this one plus
`dtolnay/rust-toolchain` and `baptiste0928/cargo-install`.

## Fixing a failure

The log ends with a diff: `-` lines are yours, `+` lines are what the compiler
produces now. Regenerate with the same versions CI uses, then commit.

Run these from your repository root, and format the temporary file before moving
it, so the same `rustfmt.toml` applies. Adjust the names to match your inputs.

Bash:

```bash
set -euo pipefail
rustup toolchain install 1.94.0 --profile minimal --component rustfmt
export RUSTUP_TOOLCHAIN=1.94.0
cargo install device-driver-cli --version '^2.1.0' --locked
ddc build --source device.ddsl --output ci_gen.rs rust
rustfmt --edition 2024 --config newline_style=Unix ci_gen.rs
mv -- ci_gen.rs src/device.rs
```

PowerShell:

```powershell
rustup toolchain install 1.94.0 --profile minimal --component rustfmt
if ($LASTEXITCODE -ne 0) { throw 'Toolchain installation failed' }
$env:RUSTUP_TOOLCHAIN = '1.94.0'
cargo install device-driver-cli --version '^2.1.0' --locked
if ($LASTEXITCODE -ne 0) { throw 'CLI installation failed' }
ddc build --source device.ddsl --output ci_gen.rs rust
if ($LASTEXITCODE -ne 0) { throw 'Generation failed' }
rustfmt --edition 2024 --config newline_style=Unix ci_gen.rs
if ($LASTEXITCODE -ne 0) { throw 'Formatting failed' }
Move-Item -LiteralPath ci_gen.rs -Destination src/device.rs -Force
```

The crate is named `device-driver-cli`, but the executable is `ddc`.

When `rust-defmt-feature` is set, add the same feature after `rust` in either
`ddc build` command above, for example `rust --rust-defmt-feature=defmt`.

If the diff looks like every line changed, your checkout has CRLF endings. Add
the `.gitattributes` rule above, then run `git add --renormalize .`.

## Common errors

| Message | Cause |
| --- | --- |
| `source must be a repository-relative path with forward slashes.` | A leading `/`, a drive letter, or `\` in the path. |
| `source does not exist.` | Not in the checkout. Check the path, or enable submodules. |
| `generated-file must be a regular file inside the checkout.` | It's a directory, or it points outside the workspace. |
| `cli-version must be a stable v2 release, such as 2.1.0.` | Ranges, prereleases, and `latest` aren't accepted. |
| `rust-toolchain must be an exact stable release, such as 1.94.0.` | `stable` and `nightly` aren't accepted. |
| No matching package while installing | `cli-version` is newer than any published v2 release. |

## Multiple devices

Add a step per device. The toolchain and compiler are installed once and reused:

```yaml
      - uses: tullom/device-driver-pregen-check@v1
        with:
          source: drivers/accel/accel.ddsl
          generated-file: drivers/accel/src/device.rs
      - uses: tullom/device-driver-pregen-check@v1
        with:
          source: drivers/gyro/gyro.ddsl
          generated-file: drivers/gyro/src/device.rs
```

## Coming from v1

This action supports v2 DDSL only. The v1 YAML input and the
`--manifest` / `--device-name` flags aren't supported, and device names now live
inside the DDSL. Swapping the CI check isn't a migration on its own: a v1 driver
also needs runtime and generated-code changes.
