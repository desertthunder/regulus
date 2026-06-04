# Summary

- [Introduction](./introduction.md)
- [Lexing, parsing, and syntax trees](./chapter_1.md)
  - [Lexing](./chapter_1/lexing.md)
  - [Parsing](./chapter_1/parsing.md)
  - [Concrete and abstract syntax trees](./chapter_1/syntax_trees.md)
  - [Gleam's grammar](./chapter_1/gleam_grammar.md)
- [Name resolution](./chapter_2.md)
  - [Lexical scope and shadowing](./chapter_2/lexical_scope.md)
  - [Symbols and scopes](./chapter_2/symbols_and_scopes.md)
  - [Namespaces and imports](./chapter_2/namespaces_and_imports.md)
  - [Patterns, visibility, and packages](./chapter_2/patterns_visibility_packages.md)
  - [The current resolver](./chapter_2/current_resolver.md)
- [Type systems](./chapter_3.md)
- [Intermediate representations and lowering](./chapter_4.md)
- [WebAssembly code generation](./chapter_5.md)
- [The Gleam Programming Language](./chapter_6.md)
  - [Modules, imports, and packages](./chapter_6/mods_imports_pkgs.md)
  - [Functions, values, and expressions](./chapter_6/functions_values_expressions.md)
  - [Custom types, records, tuples, and lists](./chapter_6/custom_types_records_tuples_lists.md)
  - [Pattern matching](./chapter_6/pattern_matching.md)
  - [Type system and inference model](./chapter_6/type_system_inference.md)
  - [Compilation](./chapter_6/compiler_notes.md)
- [Runtime value representation](./chapter_7.md)

<!--
TODO:
- A tour of Gleam
  - Gleam modules, imports, and packages
  - Gleam functions, values, and expressions
  - Gleam custom types, records, tuples, and lists
  - Gleam pattern matching
  - Gleam's type system and inference model
- WebAssembly
  - WebAssembly modules and the stack machine
  - WebAssembly text format and binary format
  - WebAssembly memory, tables, imports, and exports
  - Running WebAssembly in Wasmtime and the browser
- Compiler architecture from source to executable
- Lexing, parsing, and syntax trees
- Diagnostics and source spans
- Name resolution and scopes
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
- [Tests & Documentation](testing_docs.md)

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
  - [Project model and modules](internal/tasks/01_project_model_and_modules.md)
  - [Full Gleam syntax](internal/tasks/02_full_gleam_syntax.md)
  - [Full name resolution](internal/tasks/03_full_name_resolution.md)
  - [Gleam types and interfaces](internal/tasks/04_gleam_types_and_interfaces.md)
  - [Runtime representation](internal/tasks/05_runtime_representation.md)
  - [Pattern matching](internal/tasks/06_pattern_matching.md)
  - [Core IR for real programs](internal/tasks/07_core_ir_for_real_programs.md)
  - [WASM backend and runtime](internal/tasks/08_wasm_backend_and_runtime.md)
  - [Standard library and host interop](internal/tasks/09_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/tasks/10_cli_and_build_outputs.md)
