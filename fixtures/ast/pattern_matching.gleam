pub type Result(a, b) {
  Ok(a)
  Error(b)
}

pub type Person {
  Person(name: String, age: Int)
}

fn patterns(items, pair, person, result) {
  let assert #(first, _) = pair as "expected a tuple"

  case items, result {
    [head, ..tail], Ok(value) if value > 0 -> head
    [_, second], Error(reason) -> second
    Person(name: name, age: _), _ -> name
    #(a, b), _ -> a
    value as alias, _ -> alias
    _, _ -> 0
  }
}
