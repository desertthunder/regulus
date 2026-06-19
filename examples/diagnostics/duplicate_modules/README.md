# Duplicate modules diagnostic

This diagnostic example intentionally defines `app` in both `src/app.gleam`
and `test/app.gleam`. Project loading should reject the project before later
compiler phases run.

```sh
reggie build examples/diagnostics/duplicate_modules
```

Expected result: a duplicate module diagnostic that points at both files.
