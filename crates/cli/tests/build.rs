use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn builds_working_examples() {
    for (example, target) in [
        ("examples/scalar_project", None),
        ("examples/multi_module_project", None),
        ("examples/browser_scalar", Some("browser")),
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
"#);

    let _ = fs::remove_dir_all(out_dir);
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
        .arg("wat,ast,resolved,typed,ir")
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

    let _ = fs::remove_dir_all(out_dir);
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
    fs::write(
        &input,
        // TODO: embed as lunar.gleam
        r#"import gleam/option.{Some}
import gleam/result.{Ok}

pub type Response {
  Response(status: Int, body: String)
}

external fn request_text(input: String) -> String = "regulus/js" "request_text"
external fn describe(count: Int, ratio: Float, enabled: Bool, input: String) -> String = "regulus/js" "describe"

pub fn main(input: String) -> String {
  request_text(input)
}

pub fn describe_from_js(count: Int, ratio: Float, enabled: Bool, input: String) -> String {
  describe(count, ratio, enabled, input)
}

pub fn keep_bool(value: Bool) -> Bool {
  value
}

pub fn response() -> Response {
  Response(200, "ok")
}

pub fn names() -> List(String) {
  ["Ada", "Joe"]
}

pub fn maybe_name() -> Option(String) {
  Some("Ada")
}

pub fn result_name() -> Result(String, Int) {
  Ok("Ada")
}
"#,
    )
    .expect("write Gleam input");

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

    fs::write(
        out_dir.join("smoke.mjs"),
        // TODO: embed this as apollo.js
        r#"import { abi, call, callString, getHandle, init, releaseHandle, wrapHandle } from "./app.mjs";

if (abi.exports.describe_from_js.result !== "String") {
  throw new Error("missing export ABI metadata");
}
if (abi.imports["regulus/js.describe"].params.join(",") !== "Int,Float,Bool,String") {
  throw new Error("missing import ABI metadata");
}

await init(new URL("./app.wasm", import.meta.url), {
  "regulus/js": {
    request_text(input) {
      return `${input} from JS`;
    },
    describe(count, ratio, enabled, input) {
      return `${input}:${count}:${ratio}:${enabled}`;
    },
  },
});

const result = callString("main", "hello");
if (result !== "hello from JS") {
  throw new Error(`unexpected result: ${result}`);
}

const described = call("describe_from_js", 7n, 2.5, true, "shape");
if (described !== "shape:7:2.5:true") {
  throw new Error(`unexpected described result: ${described}`);
}

const kept = call("keep_bool", true);
if (kept !== true) {
  throw new Error(`unexpected bool result: ${kept}`);
}

const response = call("response");
if (response.tag !== "Response" || response.fields.status !== 200n || response.fields.body !== "ok") {
  throw new Error(`unexpected response: ${response.tag}`);
}

const names = call("names");
if (names.join(",") !== "Ada,Joe") {
  throw new Error(`unexpected names: ${names}`);
}

const maybeName = call("maybe_name");
if (maybeName.tag !== "Some" || maybeName.value !== "Ada") {
  throw new Error(`unexpected option: ${JSON.stringify(maybeName)}`);
}

const resultName = call("result_name");
if (resultName.tag !== "Ok" || resultName.value !== "Ada") {
  throw new Error(`unexpected result: ${JSON.stringify(resultName)}`);
}

const request = { url: "https://example.test/" };
const handle = wrapHandle(request, 42);
if (getHandle(handle, 42) !== request) {
  throw new Error("opaque handle did not round trip through the adapter table");
}
if (!releaseHandle(handle, 42)) {
  throw new Error("opaque handle was not released");
}
try {
  getHandle(handle, 42);
  throw new Error("released opaque handle lookup should fail");
} catch (error) {
  if (!String(error.message).includes("released")) {
    throw error;
  }
}
"#,
    )
    .expect("write JS smoke test");

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
