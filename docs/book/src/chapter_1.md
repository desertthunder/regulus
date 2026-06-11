# A tour of Gleam

Gleam is a statically typed functional language with a small, friendly syntax. It
runs on the Erlang virtual machine and JavaScript runtimes, and it has a package
manager, formatter, build tool, and standard library designed to be used
together.[^1]

This chapter introduces the language at a high level: modules, imports,
packages, functional programming ideas, functions, values, custom types,
records, lists, pattern matching, and type inference. The examples are based on
the Gleam language tour.[^2]

This chapter gives enough Gleam context to make the compiler chapters easier to
read. A WebAssembly compiler for Gleam has to understand modules, names,
expressions, typed data, pattern matching, and generic types before it can lower
a program to executable code.

[^1]: Gleam homepage: https://gleam.run/
[^2]: Gleam language tour: https://github.com/gleam-lang/language-tour
