# Summary

# The Book

- [Introduction](./introduction.md)
- [The Gleam Programming Language](./chapter_1.md)
  - [Modules, imports, and packages](./chapter_1/mods_imports_pkgs.md)
  - [Functional programming in Gleam](./chapter_1/functional_programming.md)
  - [Values and expressions](./chapter_1/functions_values_expressions.md)
  - [Custom types and records](./chapter_1/custom_types_records_tuples_lists.md)
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
  - [Patterns and visibility](./chapter_3/patterns_visibility_packages.md)
  - [The current resolver](./chapter_3/current_resolver.md)
- [Type checking and type inference](./chapter_4.md)
  - [Local and top-level inference](./chapter_4/local_top_level_inference.md)
  - [Algebraic data types](./chapter_4/algebraic_data_types.md)
  - [Unification and substitution](./chapter_4/unification_and_substitution.md)
  - [Calls, operators, and branches](./chapter_4/calls_operators_branches.md)
  - [Patterns and imports](./chapter_4/patterns_imports_opaque.md)
  - [Module interfaces](./chapter_4/module_interfaces.md)
- [Intermediate representations and lowering](./chapter_5.md)
- [Runtime value representation](./chapter_6.md)
- [WebAssembly code generation](./chapter_7.md)
  - [Modules and stack machine](./chapter_7/modules_stack_machine.md)
  - [Text and binary formats](./chapter_7/text_binary_format.md)
  - [Memory and module boundaries](./chapter_7/memory_tables_imports_exports.md)
  - [Wasmtime and browser](./chapter_7/running_wasmtime_browser.md)
- [Pattern matching in the compiler](./chapter_8.md)
  - [Compiling pattern matching](./chapter_8/compiling_pattern_matching.md)
  - [Binding variables from patterns](./chapter_8/binding_variables.md)
  - [Exhaustiveness diagnostics](./chapter_8/exhaustiveness_diagnostics.md)
- [Compiler architecture](./chapter_9.md)
  - [Phase boundaries](./chapter_9/phase_boundaries.md)
  - [Pass pipelines and invariants](./chapter_9/pass_pipeline_invariants.md)
  - [Project input and module graph](./chapter_9/project_to_modules.md)
  - [Artifacts and execution](./chapter_9/artifacts_execution.md)

<!--
TODO:
- Diagnostics and source spans
- Lowering and intermediate representations
  - Lowering typed and resolved syntax into a smaller IR
  - Making evaluation order, scopes, captures, and failure paths explicit
  - Distinguishing language lowering from backend-specific limitations
- Runtime value representation
- Linking modules and handling dependencies
- Standard library and host interop
- Testing a compiler end to end
-->

# Development

- [Supported subset](internal/supported_subset.md)
- [Development changelog](./CHANGELOG.md)
- [Tests & Documentation](internal/testing_docs.md)
- [Development docs]()
  - [Project model](internal/development/project_model_and_modules.md)
  - [Gleam syntax](internal/development/full_gleam_syntax.md)
  - [Name resolution](internal/development/full_name_resolution.md)
  - [Types and interfaces](internal/development/gleam_types_and_interfaces.md)
  - [Type inference](internal/development/type_and_generic_inference.md)
  - [Runtime representation](internal/development/runtime_representation.md)
  - [Pattern matching](internal/development/pattern_matching.md)
  - [Core IR](internal/development/core_ir_for_real_programs.md)
  - [WASM backend and runtime](internal/development/wasm_backend_and_runtime.md)
  - [CLI and build outputs](internal/development/cli_and_build_outputs.md)

<!--
TODO:
- This Project
  - The project pipeline in detail
  - The project CLI and build outputs
  - Extending the compiler with a new Gleam feature
-->

# Internal Docs

- [Specs]()
  - [Runtime memory](internal/specs/12_runtime_memory_and_semantics.md)
  - [Language semantics](internal/specs/13_remaining_language_semantics.md)
  - [Stdlib and host interop](internal/specs/14_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/specs/15_cli_and_build_outputs.md)
- [Active tasks]()
  - [Runtime memory](internal/tasks/12_runtime_memory_and_semantics.md)
  - [Language semantics](internal/tasks/13_remaining_language_semantics.md)
  - [Stdlib and host interop](internal/tasks/14_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/tasks/15_cli_and_build_outputs.md)
