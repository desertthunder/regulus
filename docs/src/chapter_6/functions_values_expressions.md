# Gleam functions, values, and expressions

Gleam programs are built from expressions.[^1] A function body is a block of
expressions, and the final expression is the return value:

```gleam
pub fn add_one(x: Int) -> Int {
  let one = 1
  x + one
}
```

`let` introduces a local binding. The binding is visible after it is introduced:

```gleam
let name = "Lucy"
name
```

Gleam has familiar scalar values:

```gleam
1
1.5
"hello"
True
False
Nil
```

Function calls use parentheses:

```gleam
add_one(41)
```

Arguments can have labels when a function defines them:

```gleam
pub fn replace(in string: String, each pattern: String, with replacement: String) {
  todo
}

replace(in: "one two", each: "one", with: "three")
```

Gleam also has operators, pipelines, anonymous functions, blocks, `use`,
`panic`, `todo`, and assertions. These features keep everyday code concise while
preserving Gleam's expression-oriented style.

[^1]: Gleam language tour: https://github.com/gleam-lang/language-tour
