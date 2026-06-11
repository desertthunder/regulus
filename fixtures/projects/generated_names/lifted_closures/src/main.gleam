fn call(value: Int, callback: fn(Int) -> Int) -> Int {
  callback(value)
}

pub fn run(seed: Int) -> Int {
  let add_seed = fn(value) { value + seed }
  call(1, add_seed)
}
