import gleam/int
import gleam/io
import gleam/list
import gleam/option.{None, Some}
import gleam/order.{Eq, Gt, Lt}
import gleam/result.{Error, Ok}
import gleam/string

pub fn message() -> String {
  string.append("answer: ", int.to_string(42))
}

pub fn string_size() -> Int {
  string.length(string.concat(["a", "bc"]))
}

pub fn reversed_head() -> Int {
  case list.reverse([1, 2, 3]) {
    [head, ..] -> head
    _ -> 0
  }
}

pub fn ok_value(result: Result(Int, Int)) -> Int {
  case result {
    Ok(value) -> value
    Error(_) -> 0
  }
}

pub fn option_value(option: Option(Int)) -> Int {
  case option {
    Some(value) -> value
    None -> 0
  }
}

pub fn order_rank(order: Order) -> Int {
  case order {
    Lt -> -1
    Eq -> 0
    Gt -> 1
  }
}

pub fn debug_identity() -> Int {
  io.debug(42)
}
