import gleam/bit_array
import gleam/dict
import gleam/float
import gleam/function
import gleam/int
import gleam/io
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

pub fn dict_value() -> Int {
  let values = dict.insert(dict.new(), "a", 42)
  case dict.get(values, "a") {
    Some(value) -> value
    None -> 0
  }
}

pub fn float_text() -> String {
  float.to_string(1.5)
}

pub fn constant_value() -> Int {
  function.constant(7, "ignored")
}

pub fn bits_size() -> Int {
  bit_array.bit_size(<<1, 2, 3>>)
}

pub fn bits_start() -> Bool {
  bit_array.starts_with(<<1, 2, 3>>, <<1, 2>>)
}
