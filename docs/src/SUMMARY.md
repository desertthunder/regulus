# Summary

# The Book

- [Introduction](./introduction.md)
- [Compiler architecture](./chapter_9.md)
  - [Phase boundaries](./chapter_9/phase_boundaries.md)
  - [Pass pipelines and invariants](./chapter_9/pass_pipeline_invariants.md)
  - [Project input and module graph](./chapter_9/project_to_modules.md)
  - [Artifacts and execution](./chapter_9/artifacts_execution.md)
- [The Gleam Programming Language](./chapter_1.md)
  - [Modules, imports, and packages](./chapter_1/mods_imports_pkgs.md)
  - [Functional programming in Gleam](./chapter_1/functional_programming.md)
  - [Values and expressions](./chapter_1/functions_values_expressions.md)
  - [Type Checker](./chapter_1/type_system_inference.md)
    - [Custom types and records](./chapter_1/custom_types_records_tuples_lists.md)
    - [Pattern matching](./chapter_1/pattern_matching.md)
  - [Compilation](./chapter_1/compiler_notes.md)
- [Lexing, parsing, and syntax trees](./chapter_2.md)
  - [Lexical Analysis](./chapter_2/lexical_analysis.md)
    - [Lexing](./chapter_2/lexing.md)
    - [Parsing](./chapter_2/parsing.md)
      - [Top-Down Parsers](./chapter_2/top_down.md)
      - [Bottom-Up Parsers](./chapter_2/bottom_up.md)
    - [Finite-State Machines](./chapter_2/finite_state_machines.md)
  - [Regular Expressions](./chapter_2/regex.md)
  - [Syntax Trees](./chapter_2/syntax_trees.md)
  - [Grammars](./chapter_2/gleam_grammar.md)
- [Name Resolution](./chapter_3.md)
  - [Lexical scope and shadowing](./chapter_3/lexical_scope.md)
  - [Symbols and scopes](./chapter_3/symbols_and_scopes.md)
  - [Namespaces and imports](./chapter_3/namespaces_and_imports.md)
  - [Patterns and visibility](./chapter_3/patterns_visibility_packages.md)
  - [The current resolver](./chapter_3/current_resolver.md)
- [Type Checking & Inference](./chapter_4.md)
  - [Local and top-level inference](./chapter_4/local_top_level_inference.md)
  - [Algebraic data types](./chapter_4/algebraic_data_types.md)
  - [Unification and substitution](./chapter_4/unification_and_substitution.md)
  - [Calls, operators, and branches](./chapter_4/calls_operators_branches.md)
  - [Patterns and imports](./chapter_4/patterns_imports_opaque.md)
  - [Module interfaces](./chapter_4/module_interfaces.md)
- [IR & Lowering](./chapter_5.md)
- [Runtimes (TODO)]()
  - [Runtime Value Representation](./chapter_6.md)
  - [Runtime Memory Management](./chapter_10.md)
    - [Register allocation]()
    - [The arena allocator](./chapter_10/arena_allocator.md)
    - [Growth and allocation failure](./chapter_10/growth_and_failure.md)
    - [Host pointers and reset boundaries](./chapter_10/host_pointers.md)
    - [Future collectors](./chapter_10/collector_families.md)
- [WebAssembly Code Gen](./chapter_7.md)
  - [Modules and stack machine](./chapter_7/modules_stack_machine.md)
  - [Text and binary formats](./chapter_7/text_binary_format.md)
  - [Memory and module boundaries](./chapter_7/memory_tables_imports_exports.md)
  - [Wasmtime and browser](./chapter_7/running_wasmtime_browser.md)
- [Pattern matching](./chapter_8.md)
  - [Compiling pattern matching](./chapter_8/compiling_pattern_matching.md)
  - [Binding variables](./chapter_8/binding_variables.md)
  - [Exhaustiveness diagnostics](./chapter_8/exhaustiveness_diagnostics.md)
- [WebAssembly In-Depth (TODO)]()
- [interprocedural analysis (TODO)]()
  - [Linking modules (TODO)]()
  - [handling dependencies (TODO)]()
  - [pointer analysis (TODO)]()
  - [aliasing (TODO)]()
  - [data-flow (TODO)]()
- [Code Optimization (TODO)]()
  - [Parallelization (TODO)]()
  - [Loop optimizations (TODO)]()
  - [Flow Graphs (TODO)]()
  - [Tarjan's algorithm and SCCs (TODO)]()
    - [Other Algorithms (TODO)]()

<!--
TODO:
- Diagnostics and source spans
- Lowering and intermediate representations
  - Lowering typed and resolved syntax into a smaller IR
  - Making evaluation order, scopes, captures, and failure paths explicit
  - Distinguishing language lowering from backend-specific limitations
-->

# Development

- [Supported subset](internal/supported_subset.md)
- [CHANGELOG](./CHANGELOG.md)
- [Architecture]()
  - [Core IR](internal/development/architecture/core_ir.md)
    - [Runtime representation](internal/development/architecture/runtime_representation.md)
  - [WASM backend and runtime](internal/development/architecture/wasm_backend_and_runtime.md)
  - [Runtime memory](internal/development/runtime_memory_management.md)
  - [Outputs](internal/development/cli_and_build_outputs.md)
- [Contributing]()
  - [Tests & Documentation](internal/development/contributor/testing.md)
  - [Reference]()
    - [Project model](internal/development/project_model_and_modules.md)
    - [Gleam syntax](internal/development/full_gleam_syntax.md)
    - [Name resolution](internal/development/full_name_resolution.md)
    - [Types and interfaces](internal/development/gleam_types_and_interfaces.md)
      - [Type inference](internal/development/type_and_generic_inference.md)
      - [Pattern matching](internal/development/pattern_matching.md)

<!--
  TODO:
- This Project
  - The project pipeline in detail
  - The project CLI and build outputs
  - Extending the compiler with a new Gleam feature
    -->

# Inflight Work

- [Specs]()
  - [Stdlib and host interop](internal/specs/14_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/specs/15_cli_and_build_outputs.md)
  - [Example projects](internal/specs/16_example_projects.md)
- [Task Trackers]()
  - [Stdlib and host interop](internal/tasks/14_stdlib_and_host_interop.md)
  - [CLI and build outputs](internal/tasks/15_cli_and_build_outputs.md)
  - [Example Projects]()
    - [Lustre weather SPA](internal/tasks/16_lustre_weather_example.md)
    - [Wisp Dev API](internal/tasks/17_wisp_static_data_api_example.md)
