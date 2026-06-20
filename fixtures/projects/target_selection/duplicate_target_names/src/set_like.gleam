@target(javascript)
pub type Set(element) = List(element)

@target(erlang)
pub type Set(element) {
  Set(values: List(element))
}

@target(javascript)
pub fn new() -> Set(Int) {
  []
}

@target(erlang)
pub fn new() -> Set(Int) {
  Set([])
}

@target(javascript)
pub const selected: Int = 1

@target(erlang)
pub const selected: Int = 2
