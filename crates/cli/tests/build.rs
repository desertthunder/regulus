use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn builds_working_examples() {
    for (example, target) in [
        ("examples/scalar_project", None),
        ("examples/multi_module_project", None),
        ("examples/scalar_project", Some("browser")),
    ] {
        let out_dir = unique_temp_dir("regulus_cli_example_build");
        fs::create_dir_all(&out_dir).expect("create output dir");

        let mut command = Command::new(env!("CARGO_BIN_EXE_reggie"));
        command
            .current_dir(workspace_root())
            .arg("build")
            .arg(example)
            .arg("--out-dir")
            .arg(&out_dir);
        if let Some(target) = target {
            command.arg("--target").arg(target);
        }
        let output = command.output().expect("run reggie build");

        assert!(
            output.status.success(),
            "build failed for {example}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let artifact = PathBuf::from(example)
            .file_name()
            .expect("example basename")
            .to_string_lossy()
            .replace('-', "_");
        assert!(
            out_dir.join(format!("{artifact}.wasm")).is_file(),
            "expected wasm artifact for {example} in {}",
            out_dir.display()
        );

        let _ = fs::remove_dir_all(out_dir);
    }
}

#[test]
fn global_no_color_disables_status_ansi_output() {
    let out_dir = unique_temp_dir("regulus_cli_no_color_build");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .current_dir(workspace_root())
        .arg("--no-color")
        .arg("build")
        .arg("examples/scalar_project")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie build");

    assert!(
        output.status.success(),
        "build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("wasm "), "missing status output:\n{stderr}");
    assert!(
        !stderr.contains("\u{1b}["),
        "--no-color should disable status ANSI codes: {stderr:?}"
    );

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn global_no_color_is_accepted_after_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .current_dir(workspace_root())
        .arg("list")
        .arg("examples/multi_module_project")
        .arg("--no-color")
        .output()
        .expect("run reggie list");

    assert!(
        output.status.success(),
        "list failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("project multi_module_project"),
        "missing project output:\n{stderr}"
    );
    assert!(
        !stderr.contains("\u{1b}["),
        "--no-color should disable list ANSI codes: {stderr:?}"
    );
}

#[test]
fn no_color_environment_disables_diagnostic_ansi_output() {
    let out_dir = unique_temp_dir("regulus_cli_no_color_env");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .current_dir(workspace_root())
        .env("NO_COLOR", "1")
        .arg("build")
        .arg("examples/diagnostics/duplicate_modules")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie build");

    assert!(!output.status.success(), "diagnostic example should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error could not load project"),
        "missing error output:\n{stderr}"
    );
    assert!(
        stderr.contains("diagnostic ProjectError"),
        "missing diagnostic output:\n{stderr}"
    );
    assert!(
        !stderr.contains("\u{1b}["),
        "NO_COLOR should disable diagnostic ANSI codes: {stderr:?}"
    );

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn snapshots_diagnostic_example() {
    let out_dir = unique_temp_dir("regulus_cli_example_diagnostic");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .current_dir(workspace_root())
        .arg("build")
        .arg("examples/diagnostics/duplicate_modules")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie build");

    assert!(!output.status.success(), "diagnostic example should fail");
    assert!(output.stdout.is_empty(), "unexpected stdout for diagnostic example");
    insta::assert_snapshot!(strip_ansi(&String::from_utf8_lossy(&output.stderr)), @r#"
error could not load project examples/diagnostics/duplicate_modules
diagnostic ProjectError: duplicate module `app` in examples/diagnostics/duplicate_modules/src/app.gleam and examples/diagnostics/duplicate_modules/test/app.gleam
  note: each module name must be unique across src and test
"#);

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn build_reports_missing_project_manifest_with_recovery_note() {
    let temp = unique_temp_dir("regulus_cli_missing_project");
    fs::create_dir_all(&temp).expect("create temp dir");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("build")
        .arg(&temp)
        .output()
        .expect("run reggie build");

    assert!(!output.status.success(), "missing project should fail");
    let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    assert!(
        stderr.contains("project manifest not found at"),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("pass a project directory or a path to gleam.toml"),
        "missing recovery note:\n{stderr}"
    );

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn compile_diagnostics_include_source_snippets() {
    let temp = unique_temp_dir("regulus_cli_source_diagnostic");
    fs::create_dir_all(&temp).expect("create temp dir");
    let input = temp.join("app.gleam");
    fs::write(&input, "pub fn main() -> Int { \"not an int\" }\n").expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("compile")
        .arg(&input)
        .output()
        .expect("run reggie compile");

    assert!(!output.status.success(), "compile should fail");
    let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    assert!(
        stderr.contains(&format!("{}:1:", input.display())),
        "missing source path and line in stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("pub fn main() -> Int"),
        "missing source line in stderr:\n{stderr}"
    );
    assert!(stderr.contains("^"), "missing caret label in stderr:\n{stderr}");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn build_target_override_selects_project_modules_before_type_checking() {
    let temp = unique_temp_dir("regulus_cli_build_target_override");
    let project = temp.join("project");
    fs::create_dir_all(project.join("src")).expect("create project src");
    fs::write(
        project.join("gleam.toml"),
        "name = \"target_override\"\nversion = \"1.0.0\"\ntarget = \"javascript\"\n",
    )
    .expect("write gleam.toml");
    fs::write(
        project.join("src/app.gleam"),
        r#"if javascript {
  pub fn broken() -> Unknown { missing }
}

pub fn main() -> Int { 1 }
"#,
    )
    .expect("write Gleam source");

    let default_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("build")
        .arg(&project)
        .arg("--out-dir")
        .arg(temp.join("default-out"))
        .output()
        .expect("run reggie build with config target");
    assert!(
        !default_output.status.success(),
        "javascript target should type-check selected broken group"
    );
    let default_stderr = strip_ansi(&String::from_utf8_lossy(&default_output.stderr));
    assert!(
        default_stderr.contains("src/app.gleam:2:22"),
        "missing selected target source diagnostic:\n{default_stderr}"
    );

    let out_dir = temp.join("wasmtime-out");
    let wasmtime_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("build")
        .arg(&project)
        .arg("--target")
        .arg("wasmtime")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie build with CLI target override");

    assert!(
        wasmtime_output.status.success(),
        "wasmtime override should exclude javascript-only broken group\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&wasmtime_output.stdout),
        String::from_utf8_lossy(&wasmtime_output.stderr)
    );
    assert!(out_dir.join("target_override.wasm").is_file());

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn run_decodes_managed_string_return_before_arena_reset() {
    let temp = unique_temp_dir("regulus_cli_run_string");
    fs::create_dir_all(&temp).expect("create temp dir");
    let input = temp.join("app.gleam");
    fs::write(&input, r#"pub fn main() -> String { "Ada" <> " Lovelace" }"#).expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("run")
        .arg(&input)
        .output()
        .expect("run reggie run");

    assert!(
        output.status.success(),
        "run failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Ada Lovelace\n");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn exec_alias_runs_wasmtime_exports() {
    let temp = unique_temp_dir("regulus_cli_exec_alias");
    fs::create_dir_all(&temp).expect("create temp dir");
    let input = temp.join("app.gleam");
    fs::write(
        &input,
        r#"pub fn add(left: Int, right: Int) -> Int { left + right }
pub fn pair() -> #(Int, String) { #(7, "moons") }
"#,
    )
    .expect("write Gleam input");

    let add = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("exec")
        .arg(&input)
        .arg("--function")
        .arg("add")
        .arg("40")
        .arg("2")
        .output()
        .expect("run reggie exec add");

    assert!(
        add.status.success(),
        "exec add failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&add.stdout), "42\n");

    let pair = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("exec")
        .arg(&input)
        .arg("--function")
        .arg("pair")
        .output()
        .expect("run reggie exec pair");

    assert!(
        pair.status.success(),
        "exec pair failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&pair.stdout),
        String::from_utf8_lossy(&pair.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&pair.stdout), "#(7, \"moons\")\n");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn run_rejects_non_wasmtime_targets_before_execution() {
    let temp = unique_temp_dir("regulus_cli_run_target_mismatch");
    fs::create_dir_all(&temp).expect("create temp dir");
    let input = temp.join("app.gleam");
    fs::write(&input, "pub fn main() -> Int { 1 }\n").expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("run")
        .arg(&input)
        .arg("--target")
        .arg("browser")
        .output()
        .expect("run reggie run");

    assert!(!output.status.success(), "browser run target should fail");
    let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    assert!(
        stderr.contains("target `browser` cannot be executed by Wasmtime"),
        "unexpected stderr:\n{stderr}"
    );

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn run_renders_managed_return_shapes() {
    let temp = unique_temp_dir("regulus_cli_run_managed_shapes");
    fs::create_dir_all(&temp).expect("create temp dir");
    let input = temp.join("app.gleam");
    fs::write(
        &input,
        r#"import gleam/option.{Some}
import gleam/result.{Ok}

pub type User {
  User(name: String, age: Int)
}

pub fn tuple() -> #(Int, String) { #(7, "moons") }
pub fn list() -> List(Int) { [1, 2] }
pub fn record() -> User { User(name: "Ada", age: 36) }
pub fn option() -> Option(String) { Some("Ada") }
pub fn result() -> Result(String, Int) { Ok("Ada") }
"#,
    )
    .expect("write Gleam input");

    let cases = [
        ("tuple", vec!["#(7, \"moons\")"]),
        ("list", vec!["[1 | [2 | []]]"]),
        ("record", vec!["Custom#", "\"Ada\"", "36"]),
        ("option", vec!["Custom#", "\"Ada\""]),
        ("result", vec!["Custom#", "\"Ada\""]),
    ];

    for (function, expected_parts) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
            .arg("run")
            .arg(&input)
            .arg("--function")
            .arg(function)
            .output()
            .unwrap_or_else(|error| panic!("run reggie run {function}: {error}"));

        assert!(
            output.status.success(),
            "run {function} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        for expected in expected_parts {
            assert!(
                stdout.contains(expected),
                "expected {function} output to contain {expected:?}, got {stdout:?}"
            );
        }
    }

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn build_wat_flag_matches_single_file_compile() {
    let fixture = workspace_root().join("examples/scalar_project");
    let temp = unique_temp_dir("regulus_cli_build_wat_flag");
    fs::create_dir_all(&temp).expect("create temp dir");
    let wat_path = temp.join("scalar_project.custom.wat");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("build")
        .arg(&fixture)
        .arg("--out-dir")
        .arg(temp.join("out"))
        .arg("--wat")
        .arg(&wat_path)
        .output()
        .expect("run reggie build --wat");

    assert!(
        output.status.success(),
        "build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(temp.join("out/scalar_project.wasm").is_file());
    assert!(wat_path.is_file(), "expected WAT at {}", wat_path.display());
    assert!(
        fs::read_to_string(&wat_path).expect("read WAT").contains("(module"),
        "expected WAT module text"
    );

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn build_emit_writes_wat_and_debug_artifacts_without_wasm() {
    let fixture = workspace_root().join("fixtures/projects/generated_names/dependency_module_overlap");
    let out_dir = unique_temp_dir("regulus_cli_emit_build");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("build")
        .arg(&fixture)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--emit")
        .arg("wat,ast,resolved,typed,ir,runtime,abi")
        .output()
        .expect("run reggie build");

    assert!(
        output.status.success(),
        "build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out_dir.join("dependency_module_overlap.wasm").exists());
    assert!(out_dir.join("dependency_module_overlap.wat").is_file());
    assert!(out_dir.join("dependency_module_overlap.ir.txt").is_file());
    assert!(out_dir.join("dependency_module_overlap.runtime.txt").is_file());
    assert!(out_dir.join("dependency_module_overlap.abi.txt").is_file());
    assert!(
        out_dir
            .join("dependency_module_overlap.dependency__module__overlap.main.ast.txt")
            .is_file()
    );
    assert!(
        out_dir
            .join("dependency_module_overlap.dependency__module__overlap.main.resolved.txt")
            .is_file()
    );
    assert!(
        out_dir
            .join("dependency_module_overlap.dependency__module__overlap.main.typed.txt")
            .is_file()
    );

    let runtime =
        fs::read_to_string(out_dir.join("dependency_module_overlap.runtime.txt")).expect("read runtime debug artifact");
    assert!(runtime.contains("runtime layout:"), "{runtime}");
    assert!(runtime.contains("object tags:"), "{runtime}");

    let abi = fs::read_to_string(out_dir.join("dependency_module_overlap.abi.txt")).expect("read ABI debug artifact");
    assert!(abi.contains("target: Wasmtime"), "{abi}");
    assert!(abi.contains("exports:"), "{abi}");

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn compile_emit_writes_runtime_and_abi_debug_artifacts() {
    let temp = unique_temp_dir("regulus_cli_compile_runtime_abi");
    let out_dir = temp.join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");
    let input = temp.join("app.gleam");
    fs::write(&input, "pub fn answer() -> Int { 42 }\n").expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("compile")
        .arg(&input)
        .arg("--out-dir")
        .arg(&out_dir)
        .arg("--emit")
        .arg("runtime,abi")
        .output()
        .expect("run reggie compile");

    assert!(
        output.status.success(),
        "compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!out_dir.join("app.wasm").exists());
    assert!(out_dir.join("app.runtime.txt").is_file());
    assert!(out_dir.join("app.abi.txt").is_file());

    let runtime = fs::read_to_string(out_dir.join("app.runtime.txt")).expect("read runtime artifact");
    assert!(runtime.contains("memory_limit_bytes:"), "{runtime}");

    let abi = fs::read_to_string(out_dir.join("app.abi.txt")).expect("read ABI artifact");
    assert!(abi.contains("answer -> answer"), "{abi}");
    assert!(abi.contains("result: Int as Scalar(I64)"), "{abi}");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn debug_alias_prints_tree_sitter_tree() {
    let temp = unique_temp_dir("regulus_cli_debug_ts");
    fs::create_dir_all(&temp).expect("create temp dir");
    let input = temp.join("app.gleam");
    fs::write(
        &input,
        "@external(javascript, \"regulus/js\", \"read\")\npub fn read(key: String) -> String\n",
    )
    .expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("dbg")
        .arg(&input)
        .arg("--ts")
        .output()
        .expect("run reggie dbg");

    assert!(
        output.status.success(),
        "debug failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("(source_file"), "missing source_file node: {stdout}");
    assert!(stdout.contains("(attribute"), "missing attribute node: {stdout}");
    assert!(stdout.contains("(function"), "missing function node: {stdout}");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn debug_subcommands_select_views() {
    let temp = unique_temp_dir("regulus_cli_debug_subcommands");
    fs::create_dir_all(&temp).expect("create temp dir");

    let input = temp.join("app.gleam");
    fs::write(&input, "pub fn main() -> Int { 1 }\n").expect("write Gleam input");

    let ts_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("dbg")
        .arg("ts")
        .arg(&input)
        .output()
        .expect("run reggie dbg ts");
    assert!(
        ts_output.status.success(),
        "dbg ts failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&ts_output.stdout),
        String::from_utf8_lossy(&ts_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&ts_output.stdout).contains("(source_file"),
        "missing tree output"
    );

    let ast_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("debug")
        .arg("ast")
        .arg(&input)
        .arg("--no-color")
        .output()
        .expect("run reggie debug ast");
    assert!(
        ast_output.status.success(),
        "debug ast failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&ast_output.stdout),
        String::from_utf8_lossy(&ast_output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&ast_output.stdout).contains("Function("),
        "missing AST output"
    );

    let json_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("debug")
        .arg("json")
        .arg(&input)
        .arg("--ast")
        .arg("--spans")
        .output()
        .expect("run reggie debug json");
    assert!(
        json_output.status.success(),
        "debug json failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&json_output.stdout),
        String::from_utf8_lossy(&json_output.stderr)
    );

    let json_stdout = String::from_utf8_lossy(&json_output.stdout);
    assert!(
        json_stdout.contains("\"tree_sitter\""),
        "missing tree JSON: {json_stdout}"
    );
    assert!(json_stdout.contains("\"ast\""), "missing AST JSON: {json_stdout}");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn debug_ir_prints_linked_project_ir() {
    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .current_dir(workspace_root())
        .arg("debug")
        .arg("ir")
        .arg("examples/scalar_project")
        .arg("--no-color")
        .output()
        .expect("run reggie debug ir");

    assert!(
        output.status.success(),
        "debug ir failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ir:"), "missing IR heading");
    assert!(stdout.contains("linked names:"), "missing linked names");
    assert!(
        stdout.contains("scalar_project:main.answer"),
        "missing root function linked name"
    );
    assert!(
        stdout.contains("gleam_stdlib:gleam/bool.to_string"),
        "missing stdlib source linked name"
    );
    assert!(
        !stdout.contains("__stdlib_gleam_bool_to_string"),
        "IR should not use deleted bool.to_string dispatch"
    );
}

#[test]
fn debug_without_subcommand_requires_view_flags() {
    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("dbg")
        .output()
        .expect("run reggie dbg");

    assert!(!output.status.success(), "plain dbg should fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("debug requires a subcommand"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn debug_prints_ast_spans_and_json_views() {
    let temp = unique_temp_dir("regulus_cli_debug_views");
    fs::create_dir_all(&temp).expect("create temp dir");

    let input = temp.join("app.gleam");
    fs::write(&input, "pub fn main() -> Int { 1 }\n").expect("write Gleam input");

    let ast_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("debug")
        .arg(&input)
        .arg("--ast")
        .arg("--no-color")
        .output()
        .expect("run reggie debug --ast");
    assert!(
        ast_output.status.success(),
        "debug --ast failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&ast_output.stdout),
        String::from_utf8_lossy(&ast_output.stderr)
    );

    let ast_stdout = String::from_utf8_lossy(&ast_output.stdout);
    assert!(ast_stdout.contains("ast:"), "missing AST heading: {ast_stdout}");
    assert!(ast_stdout.contains("Function("), "missing AST function: {ast_stdout}");
    assert!(
        !ast_stdout.contains("\u{1b}["),
        "--no-color should disable ANSI codes: {ast_stdout:?}"
    );

    let spans_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("debug")
        .arg(&input)
        .arg("--ts")
        .arg("--spans")
        .arg("--no-color")
        .output()
        .expect("run reggie debug --ts --spans");
    assert!(
        spans_output.status.success(),
        "debug --spans failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&spans_output.stdout),
        String::from_utf8_lossy(&spans_output.stderr)
    );

    let spans_stdout = String::from_utf8_lossy(&spans_output.stdout);
    assert!(
        spans_stdout.contains("tree-sitter:"),
        "missing tree heading: {spans_stdout}"
    );
    assert!(
        spans_stdout.contains("source_file [0.."),
        "missing root span: {spans_stdout}"
    );
    assert!(
        spans_stdout.contains("field=name"),
        "missing field names: {spans_stdout}"
    );

    let json_output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("debug")
        .arg(&input)
        .arg("--ts")
        .arg("--ast")
        .arg("--spans")
        .arg("--json")
        .output()
        .expect("run reggie debug --json");
    assert!(
        json_output.status.success(),
        "debug --json failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&json_output.stdout),
        String::from_utf8_lossy(&json_output.stderr)
    );

    let json_stdout = String::from_utf8_lossy(&json_output.stdout);
    assert!(
        json_stdout.contains("\"tree_sitter\""),
        "missing tree JSON: {json_stdout}"
    );
    assert!(
        json_stdout.contains("\"start_byte\""),
        "missing span JSON: {json_stdout}"
    );
    assert!(json_stdout.contains("\"ast\""), "missing AST JSON: {json_stdout}");

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn builds_package_owned_overlap_fixture() {
    let fixture = workspace_root().join("fixtures/projects/generated_names/dependency_module_overlap");
    let out_dir = unique_temp_dir("regulus_cli_overlap_build");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("build")
        .arg(&fixture)
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie build");

    assert!(
        output.status.success(),
        "build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        out_dir.join("dependency_module_overlap.wasm").is_file(),
        "expected wasm artifact in {}",
        out_dir.display()
    );

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn compile_bundler_target_emits_adapter_and_runs_string_smoke_test() {
    let temp = unique_temp_dir("regulus_cli_bundler_smoke");
    let out_dir = temp.join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");
    let input = temp.join("app.gleam");
    fs::write(&input, include_str!("fixtures/lunar.gleam")).expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("compile")
        .arg(&input)
        .arg("--target")
        .arg("bundler")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie compile");

    assert!(
        output.status.success(),
        "compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("app.wasm").is_file());
    assert!(out_dir.join("app.mjs").is_file());

    fs::write(out_dir.join("smoke.mjs"), include_str!("fixtures/apollo.mjs")).expect("write JS smoke test");

    let smoke = Command::new("node")
        .arg(out_dir.join("smoke.mjs"))
        .output()
        .expect("run node smoke test");

    assert!(
        smoke.status.success(),
        "node smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn compile_browser_target_emits_page_adapter_and_runs_string_smoke_test() {
    let temp = unique_temp_dir("regulus_cli_browser_smoke");
    let out_dir = temp.join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");
    let input = temp.join("app.gleam");
    fs::write(&input, include_str!("fixtures/browser_smoke.gleam")).expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("compile")
        .arg(&input)
        .arg("--target")
        .arg("browser")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie compile");

    assert!(
        output.status.success(),
        "compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("app.wasm").is_file());
    assert!(out_dir.join("app.mjs").is_file());

    let adapter = fs::read_to_string(out_dir.join("app.mjs")).expect("read browser adapter");
    assert!(adapter.contains("function createBrowserImports"), "{adapter}");
    assert!(adapter.contains("function initBrowserPage"), "{adapter}");

    fs::write(out_dir.join("smoke.mjs"), include_str!("fixtures/browser_smoke.mjs")).expect("write browser smoke test");

    let smoke = Command::new("node")
        .arg(out_dir.join("smoke.mjs"))
        .output()
        .expect("run node browser smoke test");

    assert!(
        smoke.status.success(),
        "node smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );

    let _ = fs::remove_dir_all(temp);
}

#[test]
fn compile_nodejs_target_emits_adapter_and_loads_generated_wasm() {
    let temp = unique_temp_dir("regulus_cli_nodejs_load");
    let out_dir = temp.join("out");
    fs::create_dir_all(&out_dir).expect("create output dir");

    let input = temp.join("app.gleam");
    fs::write(
        &input,
        r#"external fn env_get(key: String) -> String = "nodejs" "env.get"
pub fn main(input: String) -> String { env_get(input) }
"#,
    )
    .expect("write Gleam input");

    let output = Command::new(env!("CARGO_BIN_EXE_reggie"))
        .arg("compile")
        .arg(&input)
        .arg("--target")
        .arg("nodejs")
        .arg("--out-dir")
        .arg(&out_dir)
        .output()
        .expect("run reggie compile");

    assert!(
        output.status.success(),
        "compile failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_dir.join("app.wasm").is_file());
    assert!(out_dir.join("app.mjs").is_file());

    let adapter = fs::read_to_string(out_dir.join("app.mjs")).expect("read node adapter");
    assert!(adapter.contains("async function initNode"), "{adapter}");
    assert!(adapter.contains("function createNodeImports"), "{adapter}");

    fs::write(out_dir.join("smoke.mjs"), include_str!("fixtures/node_load.mjs")).expect("write node smoke test");

    let smoke = Command::new("node")
        .arg(out_dir.join("smoke.mjs"))
        .output()
        .expect("run node load smoke test");

    assert!(
        smoke.status.success(),
        "node smoke failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&smoke.stdout),
        String::from_utf8_lossy(&smoke.stderr)
    );

    let _ = fs::remove_dir_all(temp);
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos))
}

fn strip_ansi(input: &str) -> String {
    let mut stripped = String::new();
    let mut chars = input.chars().peekable();
    while let Some(char) = chars.next() {
        if char == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for char in chars.by_ref() {
                if char.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            stripped.push(char);
        }
    }
    stripped
}
