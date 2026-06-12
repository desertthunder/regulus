import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { afterEach, describe, expect, test } from "vitest";

const workspaceRoot = fileURLToPath(new URL("../..", import.meta.url));
const tempDirs: string[] = [];

afterEach(() => {
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { force: true, recursive: true });
  }
});

type Adapter = {
  init(wasm: URL, imports?: Record<string, Record<string, (value: string) => string>>): Promise<WebAssembly.Exports>;
  callString(name: string, ...args: string[]): string;
};

describe("JS host ABI string helpers", () => {
  test("passes JS strings to Gleam exports", async () => {
    const adapter = await compileBundlerFixture(
      "export_string_arg",
      `pub fn main(input: String) -> String { input <> " from Gleam" }`,
    );

    await adapter.init(adapter.wasmUrl);

    expect(adapter.module.callString("main", "hello")).toBe("hello from Gleam");
  });

  test("passes Gleam strings to JS imports and returns JS strings to Gleam", async () => {
    const adapter = await compileBundlerFixture(
      "import_string_arg_and_return",
      `external fn request_text(input: String) -> String = "regulus/js" "request_text"

pub fn main(input: String) -> String {
  request_text(input)
}`,
    );
    const seen: string[] = [];

    await adapter.init(adapter.wasmUrl, {
      "regulus/js": {
        request_text(input) {
          seen.push(input);
          return `${input} from JS`;
        },
      },
    });

    expect(adapter.module.callString("main", "hello")).toBe("hello from JS");
    expect(seen).toEqual(["hello"]);
  });

  test("reads strings returned from exported Gleam functions", async () => {
    const adapter = await compileBundlerFixture(
      "export_string_return",
      `pub fn greeting() -> String { "hello from Gleam" }`,
    );

    await adapter.init(adapter.wasmUrl);

    expect(adapter.module.callString("greeting")).toBe("hello from Gleam");
  });
});

async function compileBundlerFixture(name: string, source: string) {
  const temp = mkdtempSync(join(tmpdir(), `regulus-${name}-`));
  tempDirs.push(temp);
  const sourcePath = join(temp, `${name}.gleam`);
  const outDir = join(temp, "out");
  writeFileSync(sourcePath, `${source}\n`);

  try {
    execFileSync(
      "cargo",
      [
        "run",
        "-q",
        "-p",
        "compiler_cli",
        "--",
        "compile",
        sourcePath,
        "--target",
        "bundler",
        "--out-dir",
        outDir,
      ],
      { cwd: workspaceRoot, stdio: "pipe" },
    );

    const wasmUrl = pathToFileURL(join(outDir, `${name}.wasm`));
    const adapterUrl = pathToFileURL(join(outDir, `${name}.mjs`));
    adapterUrl.searchParams.set("test", `${Date.now()}-${Math.random()}`);
    const module = (await import(adapterUrl.href)) as Adapter;

    return { wasmUrl, module, init: module.init };
  } catch (error) {
    rmSync(temp, { force: true, recursive: true });
    throw error;
  }
}
