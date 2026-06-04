pub type User {
  User(name: String, age: Int)
}

fn unsupported_forms(users) {
  use user <- with_user(users)

  let bytes = <<1, 2, 3>>

  users
  |> list.map(fn(user) { user.name })
  |> list.map(label: fn(name) { name <> "!" })

  case users {
    [User(name: name, age: _), ..rest] -> #(name, rest, bytes)
    [] -> #("", [])
  }
}
