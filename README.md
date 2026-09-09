# Device-driver Pregen Check

A reusable GitHub Actions workflow for checking committed Rust generated from
device-driver v2 DDSL. It regenerates into a temporary file, formats the output,
and fails with a unified diff when the committed file is out of date. The check
never overwrites the committed file or creates a commit.

## Usage

After publishing this repository, replace `OWNER` and the example release tag
below with your repository owner and a published reference. Prefer a full commit
SHA for immutable consumer pinning. No remote repository or release is created
by this local scaffold.

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
    uses: OWNER/device-driver-pregen-check/.github/workflows/pregen-check.yml@v1.0.0
    with:
      source: device.ddsl
      generated-file: src/device.rs
      cli-version: '2.1.0'
      rust-toolchain: '1.94.0'
```

Call this at job level, not under `steps`. The reusable workflow owns the runner
and steps and checks out the **calling repository**. Keep triggers and concurrency
in the caller; the reusable workflow does not define a competing concurrency group.
Neither an `action.yml` file nor GitHub's "Template repository" setting is needed.

## Inputs

| Input | Type | Default | Meaning |
| --- | --- | --- | --- |
| `source` | string | required | Existing DDSL source, relative to the caller's repository root |
| `generated-file` | string | `src/device.rs` | Existing committed Rust file to compare against |
| `cli-version` | string | `2.1.0` | Minimum stable v2 CLI version, at least `2.1.0` |
| `rust-toolchain` | string | `1.94.0` | Exact stable Rust release, at least 1.94.0 and the selected CLI's MSRV |
| `submodules` | boolean | `false` | Check out the caller's submodules |

Paths use forward slashes. Spaces and shell metacharacters are treated literally.
Absolute paths, directories, and paths resolving outside the checkout are rejected.
Generation runs on an Ubuntu GitHub-hosted runner. The CLI installation is cached
and uses Cargo's caret requirement with `--locked`. For example, `cli-version:
'2.1.0'` selects the newest available release from `>=2.1.0, <3.0.0`; the cache
also includes the toolchain. Version ranges, prereleases, and `latest` are rejected
as inputs.

The root `rustfmt.toml` or `.rustfmt.toml` controls formatting. Directory-specific
formatting configurations next to the expected file are not used. Both generation
and the final formatting pass run from the repository root, with the temporary
output also at that root. The final pass uses edition 2024 and LF newlines.

Comparison is exact: whitespace and line-ending differences fail too. For
consistent checkouts, include this rule in the caller's `.gitattributes`:

```gitattributes
*.rs text eol=lf
```

Ordinary use requires only `contents: read`, with no additional secrets. The caller
must allow this workflow and its pinned actions in its Actions settings. A public
workflow repository is simplest for public consumers. Private workflow repositories
need appropriate access settings; `submodules: true` does not grant credentials for
otherwise inaccessible private submodules.

## Regenerate Locally

Use the same CLI and Rust versions as CI. Run from the caller's repository root;
format the temporary root-level file before moving it to the expected location.
Adjust the source and output names to match your workflow inputs.

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

The package is named `device-driver-cli`, but its v2 executable is `ddc`.
Device names are declared inside DDSL. The v1 YAML input and the old
`--manifest` / `--device-name` command are not supported by this workflow.
Migrating an existing v1 driver also requires compatible runtime and generated-code
changes; switching its workflow alone is not a runtime migration.

## Development

The local toolchain is pinned in `rust-toolchain.toml`. Install the compiler into
the ignored `.tools` directory and run the tests from this repository's root:

```text
cargo +1.94.0 install device-driver-cli --version '=2.1.0' --locked --root .tools
cargo test --locked
```

Tests need Rust 1.94.0 with rustfmt and Bash with GNU coreutils. On Windows they use
Git Bash from the standard Git installation; set `BASH_PATH` for a custom location.
Tests prefer `.tools/bin/ddc` over any compiler already on `PATH`. The Rust test
dependencies are development-only; consumers need no Cargo project.

The test harness parses the workflow YAML and executes its actual version-validation
and generation scripts in disposable checkouts. It covers matching and stale output,
changed and invalid DDSL, missing files, unsafe paths, shell metacharacters,
nondefault formatting, formatter errors, exact line endings, cleanup, and preservation
of the expected file. Baselines are not regenerated by the tests.

Workflow CI has three jobs: actionlint, these behavior tests, and a same-commit call
to the reusable workflow using `tests/fixtures/device.ddsl` and its Rust baseline.
Lint locally with actionlint 1.7.7; CI installs it with:

```bash
go install github.com/rhysd/actionlint/cmd/actionlint@v1.7.7
"$(go env GOPATH)/bin/actionlint"
```

To update the fixture intentionally, use the regeneration procedure above with
`tests/fixtures/device.ddsl` as the source and `tests/fixtures/device.rs` as the
destination, then rerun the tests. Review generated changes whenever upgrading
either the compiler or the formatter, even for patch releases.

## Publication

1. Create a public GitHub repository under the desired owner and configure its remote.
2. Review and commit the scaffold, then push it and confirm all three CI jobs pass.
3. Test a cross-repository caller, including a stale-output failure.
4. Publish an initial workflow release such as `v1.0.0`; optionally maintain a `v1`
   convenience tag while recommending full commit SHAs to consumers.

The workflow's release version is independent of the device-driver version it
installs: workflow `v1.0.0` can use device-driver `2.1.0`.