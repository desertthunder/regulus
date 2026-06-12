use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
        r#"external fn request_text(input: String) -> String = "regulus/js" "request_text"

pub fn main(input: String) -> String {
  request_text(input)
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
        r#"import { init, callString } from "./app.mjs";

await init(new URL("./app.wasm", import.meta.url), {
  "regulus/js": {
    request_text(input) {
      return `${input} from JS`;
    },
  },
});

const result = callString("main", "hello");
if (result !== "hello from JS") {
  throw new Error(`unexpected result: ${result}`);
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
