const BUNDLER_ADAPTER: &str = include_str!("adapter.js");

pub fn bundler_adapter(wasm_file: &str) -> String {
    BUNDLER_ADAPTER.replace("__WASM_FILE__", wasm_file)
}
