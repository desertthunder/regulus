pub type Outcome {
  Ok(Bool)
  Error(String)
}

pub type Person {
  Person(name: String, age: Int)
}

fn main(result, person) {
  case result {
    Ok(value) if value -> value
    Error(reason) -> False
  }

  case person {
    Person(name:, age: _) -> name
  }
}
