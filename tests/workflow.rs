use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::sync::OnceLock;

use serde::Deserialize;
use serde_yml::Value;
use tempfile::TempDir;

#[derive(Debug, Deserialize)]
struct Action {
    inputs: BTreeMap<String, Input>,
    runs: ActionRuns,
}

#[derive(Debug, Deserialize)]
struct ActionRuns {
    using: String,
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
struct Input {
    #[serde(default)]
    default: Option<Value>,
    #[serde(default)]
    required: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct Step {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    shell: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    run: Option<String>,
    #[serde(default)]
    uses: Option<String>,
    #[serde(default)]
    with: BTreeMap<String, Value>,
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn action() -> &'static Action {
    static ACTION: OnceLock<Action> = OnceLock::new();
    ACTION.get_or_init(|| {
        let contents = fs::read_to_string(repository_root().join("action.yml")).expect("read composite action");
        serde_yml::from_str(&contents).expect("parse composite action")
    })
}

struct RunResult {
    status: ExitStatus,
    output: String,
}

fn bash_path() -> OsString {
    env::var_os("BASH_PATH").unwrap_or_else(|| {
        if cfg!(windows) {
            PathBuf::from(env::var_os("ProgramFiles").unwrap_or_else(|| OsString::from("C:/Program Files")))
                .join("Git/bin/bash.exe")
                .into_os_string()
        } else {
            OsString::from("bash")
        }
    })
}

fn run_script(script: &str, workspace: &Path, overrides: &[(&str, &str)]) -> RunResult {
    let inputs = &action().inputs;
    let rust_toolchain = inputs["rust-toolchain"]
        .default
        .as_ref()
        .and_then(Value::as_str)
        .unwrap();
    let cli_version = inputs["cli-version"].default.as_ref().and_then(Value::as_str).unwrap();
    let rust_defmt_feature = inputs["rust-defmt-feature"]
        .default
        .as_ref()
        .and_then(Value::as_str)
        .unwrap();
    let mut paths = vec![repository_root().join(".tools/bin")];
    if let Some(inherited_path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&inherited_path));
    }

    let output = Command::new(bash_path())
        .args(["--noprofile", "--norc", "-c", script])
        .current_dir(workspace)
        .env("PATH", env::join_paths(paths).expect("join executable paths"))
        .env("RUSTUP_TOOLCHAIN", rust_toolchain)
        .env("CLI_VERSION", cli_version)
        .env("GITHUB_WORKSPACE", workspace.to_string_lossy().replace('\\', "/"))
        .env("SOURCE_FILE", "tests/fixtures/device.ddsl")
        .env("GENERATED_FILE", "tests/fixtures/device.rs")
        .env("RUST_DEFMT_FEATURE", rust_defmt_feature)
        .envs(overrides.iter().copied())
        .output()
        .expect("run action script with Bash");

    RunResult {
        status: output.status,
        output: format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    }
}

fn run_step(id: &str, workspace: &Path, overrides: &[(&str, &str)]) -> RunResult {
    let step = action()
        .runs
        .steps
        .iter()
        .find(|step| step.id.as_deref() == Some(id))
        .unwrap_or_else(|| panic!("action step {id:?}"));
    run_script(
        step.run.as_deref().expect("action step has a run script"),
        workspace,
        overrides,
    )
}

struct Fixture {
    _directory: TempDir,
    workspace: PathBuf,
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create fixture directory");
    for entry in fs::read_dir(source).expect("read fixture directory") {
        let entry = entry.expect("read fixture entry");
        let destination_path = destination.join(entry.file_name());
        if entry.file_type().expect("read fixture file type").is_dir() {
            copy_directory(&entry.path(), &destination_path);
        } else {
            fs::copy(entry.path(), destination_path).expect("copy fixture file");
        }
    }
}

fn copy_fixture() -> Fixture {
    let directory = tempfile::Builder::new()
        .prefix("ddc pregen ")
        .tempdir()
        .expect("create temporary directory");
    let workspace = directory.path().join("checkout");
    fs::create_dir(&workspace).expect("create temporary checkout");
    copy_directory(
        &repository_root().join("tests/fixtures"),
        &workspace.join("tests/fixtures"),
    );
    fs::copy(repository_root().join("rustfmt.toml"), workspace.join("rustfmt.toml"))
        .expect("copy rustfmt configuration");
    Fixture {
        _directory: directory,
        workspace,
    }
}

fn run_check(workspace: &Path, overrides: &[(&str, &str)]) -> RunResult {
    let expected_file = workspace.join("tests/fixtures/device.rs");
    let before = fs::read(&expected_file).expect("read expected generated file");
    let result = run_step("check", workspace, overrides);
    assert_eq!(
        fs::read(&expected_file).expect("reread expected generated file"),
        before,
        "the check must not overwrite the expected file"
    );
    let temporary_outputs: Vec<_> = fs::read_dir(workspace)
        .expect("read temporary checkout")
        .map(|entry| entry.expect("read temporary checkout entry").file_name())
        .filter(|name| name.to_string_lossy().starts_with(".device-driver-pregen."))
        .collect();
    assert!(
        temporary_outputs.is_empty(),
        "temporary outputs remain: {temporary_outputs:?}"
    );
    result
}

#[test]
fn composite_action_has_pinned_dependencies_and_expected_inputs() {
    let action = action();
    let inputs = &action.inputs;
    assert_eq!(action.runs.using, "composite");
    assert_eq!(inputs.len(), 5);
    assert_eq!(inputs["source"].required, Some(true));
    assert_eq!(
        inputs["generated-file"].default.as_ref().and_then(Value::as_str),
        Some("src/device.rs")
    );
    assert_eq!(
        inputs["cli-version"].default.as_ref().and_then(Value::as_str),
        Some("2.1.1")
    );
    assert_eq!(
        inputs["rust-toolchain"].default.as_ref().and_then(Value::as_str),
        Some("1.94.0")
    );
    assert_ne!(inputs["rust-defmt-feature"].required, Some(true));
    assert_eq!(
        inputs["rust-defmt-feature"].default.as_ref().and_then(Value::as_str),
        Some("")
    );

    for step in action.runs.steps.iter().filter(|step| step.uses.is_some()) {
        let reference = step
            .uses
            .as_deref()
            .unwrap()
            .rsplit_once('@')
            .map(|(_, reference)| reference);
        assert!(
            reference.is_some_and(|reference| {
                reference.len() == 40
                    && reference
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }),
            "action is not pinned to a full commit: {:?}",
            step.uses
        );
    }
}

#[test]
fn composite_action_configures_its_steps_without_checkout() {
    let action = action();
    assert!(!action.inputs.contains_key("submodules"));
    let steps = &action.runs.steps;
    for step in steps {
        assert_eq!(step.env["RUSTUP_TOOLCHAIN"], "${{ inputs.rust-toolchain }}");
        if step.run.is_some() {
            assert_eq!(step.shell.as_deref(), Some("bash"));
        }
        assert!(
            step.uses
                .as_deref()
                .is_none_or(|uses| !uses.starts_with("actions/checkout@")),
            "checkout belongs to the calling job"
        );
    }

    let validate = steps
        .iter()
        .find(|step| step.id.as_deref() == Some("validate"))
        .expect("version validation step");
    assert_eq!(validate.env["CLI_VERSION"], "${{ inputs.cli-version }}");

    let rust_setup = steps
        .iter()
        .find(|step| {
            step.uses
                .as_deref()
                .is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
        })
        .expect("Rust and rustfmt install step");
    assert_eq!(
        rust_setup.with["toolchain"].as_str(),
        Some("${{ inputs.rust-toolchain }}")
    );
    assert_eq!(rust_setup.with["components"].as_str(), Some("rustfmt"));

    let install = steps
        .iter()
        .find(|step| step.with.get("crate").and_then(Value::as_str) == Some("device-driver-cli"))
        .expect("device-driver-cli install step");
    assert_eq!(install.with["locked"].as_bool(), Some(true));
    assert_eq!(install.with["version"].as_str(), Some("^${{ inputs.cli-version }}"));
    assert_eq!(
        install.with["cache-key"].as_str(),
        Some("rust-${{ inputs.rust-toolchain }}")
    );

    let check = steps
        .iter()
        .find(|step| step.id.as_deref() == Some("check"))
        .expect("generation and comparison step");
    assert_eq!(check.env["SOURCE_FILE"], "${{ inputs.source }}");
    assert_eq!(check.env["GENERATED_FILE"], "${{ inputs.generated-file }}");
    assert_eq!(check.env["RUST_DEFMT_FEATURE"], "${{ inputs.rust-defmt-feature }}");
}

#[test]
fn composite_smoke_test_regenerates_its_baseline_with_matching_tooling() {
    let contents = fs::read_to_string(repository_root().join(".github/workflows/test.yml")).unwrap();
    let workflow: Value = serde_yml::from_str(&contents).expect("parse test workflow");
    let job = &workflow["jobs"]["composite"];
    let steps: Vec<Step> = serde_yml::from_value(job["steps"].clone()).expect("parse composite test steps");
    let inputs = &action().inputs;
    let toolchain = inputs["rust-toolchain"]
        .default
        .as_ref()
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(job["env"]["RUSTUP_TOOLCHAIN"].as_str(), Some(toolchain));

    let rust_setup = steps
        .iter()
        .find(|step| {
            step.uses
                .as_deref()
                .is_some_and(|uses| uses.starts_with("dtolnay/rust-toolchain@"))
        })
        .expect("smoke-test Rust and rustfmt install step");
    assert_eq!(rust_setup.with["toolchain"].as_str(), Some(toolchain));
    assert_eq!(rust_setup.with["components"].as_str(), Some("rustfmt"));

    let install = steps
        .iter()
        .find(|step| step.with.get("crate").and_then(Value::as_str) == Some("device-driver-cli"))
        .expect("smoke-test compiler install step");
    let cli_version = inputs["cli-version"].default.as_ref().and_then(Value::as_str).unwrap();
    assert_eq!(
        install.with["version"].as_str(),
        Some(format!("^{cli_version}").as_str())
    );
    assert_eq!(install.with["locked"].as_bool(), Some(true));
    assert_eq!(
        install.with["cache-key"].as_str(),
        Some(format!("rust-{toolchain}").as_str())
    );

    let prepare_index = steps
        .iter()
        .position(|step| step.id.as_deref() == Some("prepare-fixture"))
        .expect("smoke-test baseline generation step");
    let check_index = steps
        .iter()
        .position(|step| step.uses.as_deref() == Some("./"))
        .expect("local composite action invocation");
    assert!(
        prepare_index < check_index,
        "generate the baseline before running the action"
    );
    assert_eq!(
        steps[check_index].with["source"].as_str(),
        Some("tests/fixtures/device.ddsl")
    );
    assert_eq!(
        steps[check_index].with["generated-file"].as_str(),
        Some("tests/fixtures/device.rs")
    );

    let fixture = copy_fixture();
    fs::write(
        fixture.workspace.join("tests/fixtures/device.rs"),
        "// Stale baseline\n",
    )
    .unwrap();
    let preparation = run_script(
        steps[prepare_index].run.as_deref().expect("baseline generation script"),
        &fixture.workspace,
        &[],
    );
    assert!(preparation.status.success(), "{}", preparation.output);
    let result = run_check(&fixture.workspace, &[]);
    assert!(result.status.success(), "{}", result.output);
}

#[test]
fn supported_minimum_versions_pass_validation() {
    let cases: &[&[(&str, &str)]] = &[
        &[],
        &[("CLI_VERSION", "2.1.1")],
        &[("CLI_VERSION", "2.12.3"), ("RUSTUP_TOOLCHAIN", "1.95.1")],
    ];
    for overrides in cases {
        let result = run_step("validate", &repository_root(), overrides);
        assert!(result.status.success(), "{}", result.output);
    }
}

#[test]
fn unsupported_versions_fail_validation() {
    let cases: &[&[(&str, &str)]] = &[
        &[("CLI_VERSION", "1.0.9")],
        &[("CLI_VERSION", "2.0.99")],
        &[("CLI_VERSION", "^2.1.0")],
        &[("CLI_VERSION", "2.1.0-rc.1")],
        &[("CLI_VERSION", "2.1.0; exit 0")],
        &[("RUSTUP_TOOLCHAIN", "stable")],
        &[("RUSTUP_TOOLCHAIN", "nightly")],
        &[("RUSTUP_TOOLCHAIN", "1.93.0")],
        &[("RUSTUP_TOOLCHAIN", "1.94")],
    ];
    for overrides in cases {
        let result = run_step("validate", &repository_root(), overrides);
        assert_eq!(result.status.code(), Some(1), "{overrides:?}\n{}", result.output);
        assert!(result.output.contains("::error::"), "{}", result.output);
    }
}

#[test]
fn matching_ddsl_and_generated_rust_pass_without_changing_the_baseline() {
    let fixture = copy_fixture();
    let result = run_check(&fixture.workspace, &[]);
    assert!(result.status.success(), "{}", result.output);
    assert!(result.output.contains("device-driver-cli 2.1.0"), "{}", result.output);
    assert!(
        !fs::read_to_string(fixture.workspace.join("tests/fixtures/device.rs"))
            .unwrap()
            .contains('\r')
    );
}

#[test]
fn defmt_generation_matches_the_requested_feature_and_requires_opt_in() {
    for feature in ["defmt", "custom-defmt"] {
        let fixture = copy_fixture();
        let overrides = [("RUST_DEFMT_FEATURE", feature)];
        let generation = run_script(
            concat!(
                "set -euo pipefail\n",
                "ddc build --source tests/fixtures/device.ddsl --output ci_gen.rs rust ",
                "--rust-defmt-feature=\"$RUST_DEFMT_FEATURE\"\n",
                "rustfmt --edition 2024 --config newline_style=Unix ci_gen.rs\n",
            ),
            &fixture.workspace,
            &overrides,
        );
        assert!(generation.status.success(), "{}", generation.output);
        let generated_file = fixture.workspace.join("ci_gen.rs");
        let contents = fs::read_to_string(&generated_file).unwrap();
        assert!(
            contents.contains(&format!("#[cfg(feature = \"{feature}\")]")),
            "{contents}"
        );
        assert!(contents.contains("impl defmt::Format"), "{contents}");
        fs::rename(generated_file, fixture.workspace.join("tests/fixtures/device.rs")).unwrap();

        let result = run_check(&fixture.workspace, &overrides);
        assert!(result.status.success(), "{feature:?}\n{}", result.output);

        let result = run_check(&fixture.workspace, &[]);
        assert_eq!(result.status.code(), Some(1), "{feature:?}\n{}", result.output);
        assert!(result.output.contains("@@"), "{}", result.output);
    }
}

#[test]
fn defmt_generation_rejects_a_baseline_without_defmt() {
    let fixture = copy_fixture();
    let result = run_check(&fixture.workspace, &[("RUST_DEFMT_FEATURE", "defmt")]);
    assert_eq!(result.status.code(), Some(1), "{}", result.output);
    assert!(result.output.contains("@@"), "{}", result.output);
}

#[test]
fn stale_generated_rust_fails_with_a_diff_and_is_not_overwritten() {
    let fixture = copy_fixture();
    let expected_file = fixture.workspace.join("tests/fixtures/device.rs");
    let mut contents = fs::read_to_string(&expected_file).unwrap();
    contents.push('\n');
    fs::write(&expected_file, contents).unwrap();
    let result = run_check(&fixture.workspace, &[]);
    assert_eq!(result.status.code(), Some(1), "{}", result.output);
    assert!(result.output.contains("@@"), "{}", result.output);
}

#[test]
fn changed_ddsl_fails_until_its_generated_baseline_is_updated() {
    let fixture = copy_fixture();
    let source_file = fixture.workspace.join("tests/fixtures/device.ddsl");
    let contents = fs::read_to_string(&source_file)
        .unwrap()
        .replace("address: 0x01", "address: 0x02");
    fs::write(source_file, contents).unwrap();
    let result = run_check(&fixture.workspace, &[]);
    assert_eq!(result.status.code(), Some(1), "{}", result.output);
    assert!(result.output.contains("@@"), "{}", result.output);
}

#[test]
fn invalid_ddsl_fails_and_removes_temporary_output() {
    let fixture = copy_fixture();
    fs::write(
        fixture.workspace.join("tests/fixtures/device.ddsl"),
        "device { invalid\n",
    )
    .unwrap();
    let result = run_check(&fixture.workspace, &[]);
    assert_eq!(result.status.code(), Some(1), "{}", result.output);
    assert!(
        result.output.to_ascii_lowercase().contains("error"),
        "{}",
        result.output
    );
}

#[test]
fn missing_files_and_directory_inputs_fail() {
    let fixture = copy_fixture();
    let cases: &[&[(&str, &str)]] = &[
        &[("SOURCE_FILE", "missing.ddsl")],
        &[("GENERATED_FILE", "missing.rs")],
        &[("SOURCE_FILE", "tests/fixtures")],
        &[("GENERATED_FILE", "tests/fixtures")],
    ];
    for overrides in cases {
        let result = run_check(&fixture.workspace, overrides);
        assert_eq!(result.status.code(), Some(1), "{}", result.output);
        assert!(result.output.contains("::error::"), "{}", result.output);
    }
}

#[test]
fn absolute_paths_and_paths_escaping_the_checkout_are_rejected() {
    let fixture = copy_fixture();
    let outside_directory = fixture.workspace.parent().unwrap();
    fs::copy(
        fixture.workspace.join("tests/fixtures/device.ddsl"),
        outside_directory.join("outside.ddsl"),
    )
    .unwrap();
    fs::copy(
        fixture.workspace.join("tests/fixtures/device.rs"),
        outside_directory.join("outside.rs"),
    )
    .unwrap();
    let absolute_source = fixture
        .workspace
        .join("tests/fixtures/device.ddsl")
        .to_string_lossy()
        .replace('\\', "/");
    let cases = [
        ("SOURCE_FILE", "../outside.ddsl"),
        ("GENERATED_FILE", "../outside.rs"),
        ("SOURCE_FILE", absolute_source.as_str()),
        ("SOURCE_FILE", "tests\\fixtures\\device.ddsl"),
    ];
    for (name, value) in cases {
        let overrides = [(name, value)];
        let result = run_check(&fixture.workspace, &overrides);
        assert_eq!(result.status.code(), Some(1), "{overrides:?}\n{}", result.output);
        assert!(result.output.contains("::error::"), "{}", result.output);
    }
}

#[test]
fn spaces_and_shell_metacharacters_in_relative_paths_remain_literal() {
    let fixture = copy_fixture();
    let directory = "fixture files; $(touch INJECTED)";
    copy_directory(
        &fixture.workspace.join("tests/fixtures"),
        &fixture.workspace.join(directory),
    );
    let source_file = format!("{directory}/device.ddsl");
    let generated_file = format!("{directory}/device.rs");
    let overrides = [
        ("SOURCE_FILE", source_file.as_str()),
        ("GENERATED_FILE", generated_file.as_str()),
    ];
    let result = run_check(&fixture.workspace, &overrides);
    assert!(result.status.success(), "{}", result.output);
    assert!(!fixture.workspace.join("INJECTED").exists());
}

#[test]
fn generation_and_final_formatting_honor_the_checkout_root_rustfmt_config() {
    let fixture = copy_fixture();
    let expected_file = fixture.workspace.join("tests/fixtures/device.rs");
    let before = fs::read_to_string(&expected_file).unwrap();
    fs::write(fixture.workspace.join("rustfmt.toml"), "max_width = 60\n").unwrap();
    let formatting = run_script(
        "rustfmt --edition 2024 --config newline_style=Unix tests/fixtures/device.rs",
        &fixture.workspace,
        &[],
    );
    assert!(formatting.status.success(), "{}", formatting.output);
    assert_ne!(fs::read_to_string(&expected_file).unwrap(), before);
    let result = run_check(&fixture.workspace, &[]);
    assert!(result.status.success(), "{}", result.output);
}

#[test]
fn nested_output_directory_does_not_override_the_root_formatting_convention() {
    let fixture = copy_fixture();
    fs::write(
        fixture.workspace.join("tests/fixtures/rustfmt.toml"),
        "max_width = 60\n",
    )
    .unwrap();
    let result = run_check(&fixture.workspace, &[]);
    assert!(result.status.success(), "{}", result.output);
}

#[test]
fn formatter_failure_fails_the_check_and_cleans_up() {
    let fixture = copy_fixture();
    fs::write(fixture.workspace.join("rustfmt.toml"), "max_width = \"invalid\"\n").unwrap();
    let result = run_check(&fixture.workspace, &[]);
    assert_eq!(result.status.code(), Some(1), "{}", result.output);
    assert!(result.output.contains("max_width"), "{}", result.output);
}

#[test]
fn line_ending_differences_fail_the_exact_comparison() {
    let fixture = copy_fixture();
    let expected_file = fixture.workspace.join("tests/fixtures/device.rs");
    let contents = fs::read_to_string(&expected_file).unwrap().replace('\n', "\r\n");
    fs::write(&expected_file, contents).unwrap();
    let result = run_check(&fixture.workspace, &[]);
    assert_eq!(result.status.code(), Some(1), "{}", result.output);
    assert!(result.output.contains("@@"), "{}", result.output);
}
