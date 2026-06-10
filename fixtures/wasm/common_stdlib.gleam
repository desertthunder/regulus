import gleam/bool
import gleam/dict
import gleam/float
import gleam/function
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

pub fn bool_text() -> String {
  bool.to_string(True)
}

pub fn dict_value() -> Int {
  let values = dict.insert(dict.new(), "a", 42)
  case dict.get(values, "a") {
    Some(value) -> value
    None -> 0
  }
}

pub fn float_larger() -> Float {
  float.max(1.5, float.negate(-2.5))
}

pub fn same_value() -> Int {
  function.identity(9)
}
