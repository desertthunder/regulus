# Browser scalar

This working example builds a browser-target Wasm module without host imports.
The checked-in `host.js` shows the minimal browser instantiation boundary.

```sh
gleam-wasm build examples/browser_scalar --out-dir build/examples
```

The command writes `build/examples/browser_scalar.wasm`.
