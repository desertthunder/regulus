@target(javascript)
pub const answer = 42
pub type Person { Person(name: String, age: Int) }
type UserId = Int
external fn alert(message: String) -> Nil = "window" "alert"

fn main(items) {
  let #(first, _) = #(1, 2)
  case items {
    [head, ..tail] -> head
    _ -> first
  }
}
