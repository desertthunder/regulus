# Summary

- [Introduction](./introduction.md)
- [The Gleam Programming Language](./chapter_1.md)
  - [Modules, imports, and packages](./chapter_1/mods_imports_pkgs.md)
  - [Functional programming in Gleam](./chapter_1/functional_programming.md)
  - [Functions, values, and expressions](./chapter_1/functions_values_expressions.md)
  - [Custom types, records, tuples, and lists](./chapter_1/custom_types_records_tuples_lists.md)
  - [Pattern matching](./chapter_1/pattern_matching.md)
  - [Type system and inference model](./chapter_1/type_system_inference.md)
  - [Compilation](./chapter_1/compiler_notes.md)
- [Lexing, parsing, and syntax trees](./chapter_2.md)
  - [Lexing](./chapter_2/lexing.md)
  - [Parsing](./chapter_2/parsing.md)
  - [Concrete and abstract syntax trees](./chapter_2/syntax_trees.md)
  - [Gleam's grammar](./chapter_2/gleam_grammar.md)
- [Name resolution](./chapter_3.md)
  - [Lexical scope and shadowing](./chapter_3/lexical_scope.md)
  - [Symbols and scopes](./chapter_3/symbols_and_scopes.md)
  - [Namespaces and imports](./chapter_3/namespaces_and_imports.md)
  - [Patterns, visibility, and packages](./chapter_3/patterns_visibility_packages.md)
  - [The current resolver](./chapter_3/current_resolver.md)
- [Type systems](./chapter_4.md)
- [Intermediate representations and lowering](./chapter_5.md)
- [Runtime value representation](./chapter_6.md)
- [WebAssembly code generation](./chapter_7.md)

<!--
TODO:
- WebAssembly
  - WebAssembly modules and the stack machine
  - WebAssembly text format and binary format
  - WebAssembly memory, tables, imports, and exports
  - Running WebAssembly in Wasmtime and the browser
- Compiler architecture from source to executable
- Diagnostics and source spans
- Type checking and type inference
- Lowering and intermediate representations
- Runtime value representation
- Pattern Matching
  - Compiling pattern matching
- Code generation for WebAssembly
- Linking modules and handling dependencies
- Standard library and host interop
- Testing a compiler end to end
- This Project
  - The project pipeline in detail
  - The project CLI and build outputs
  - Extending the compiler with a new Gleam feature

-->

# Development

- [Supported subset](internal/supported_subset.md)
- [Development changelog](CHANGELOG.md)
- [Tests & Documentation](internal/testing_docs.md)

# Internal Docs

- [Specs]()
  - [Project model and modules](internal/specs/01_project_model_and_modules.md)
  - [Full Gleam syntax](internal/specs/02_full_gleam_syntax.md)
  - [Full name resolution](internal/specs/03_full_name_resolution.md)
  - [Gleam types and interfaces](internal/specs/04_gleam_types_and_interfaces.md)
  - [Runtime representation](internal/specs/05_runtime_representation.md)
  - [Pattern matching](internal/specs/06_pattern_matching.md)
  - [Core IR for real programs](internal/specs/07_core_ir_for_real_programs.md)
  - [WASM backend and runtime](internal/specs/08_wasm_backend_and_runtime.md)
  - [Standard library and host interop](internal/specs/09_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/specs/10_cli_and_build_outputs.md)
- [Tasks]()
  - [WASM backend and runtime](internal/tasks/08_wasm_backend_and_runtime.md)
  - [Standard library and host interop](internal/tasks/09_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/tasks/10_cli_and_build_outputs.md)
