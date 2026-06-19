# Multi-module project

This working example proves that a project build can resolve a same-project
import and link the lowered modules into one Wasm artifact.

```sh
reggie build examples/multi_module_project --out-dir build/examples
```

The command writes `build/examples/multi_module_project.wasm`.
