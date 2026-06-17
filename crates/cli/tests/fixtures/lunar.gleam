import gleam/option.{Some}
import gleam/result.{Ok}

pub type Response {
  Response(status: Int, body: String)
}

pub opaque type Request {
  Request
}

external fn request_text(input: String) -> String = "regulus/js" "request_text"
external fn describe(count: Int, ratio: Float, enabled: Bool, input: String) -> String = "regulus/js" "describe"
external fn pass_request(input: Request) -> Request = "regulus/js" "pass_request"

pub fn main(input: String) -> String {
  request_text(input)
}

pub fn describe_from_js(count: Int, ratio: Float, enabled: Bool, input: String) -> String {
  describe(count, ratio, enabled, input)
}

pub fn keep_bool(value: Bool) -> Bool {
  value
}

pub fn response() -> Response {
  Response(200, "ok")
}

pub fn names() -> List(String) {
  ["Ada", "Joe"]
}

pub fn maybe_name() -> Option(String) {
  Some("Ada")
}

pub fn result_name() -> Result(String, Int) {
  Ok("Ada")
}

pub fn round_trip_request(input: Request) -> Request {
  pass_request(input)
}
