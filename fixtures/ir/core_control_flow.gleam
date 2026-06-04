type Outcome {
  Done(Int)
  Failed(Int)
}

fn choose(result: Outcome, keep: Bool) -> Int {
  let assert first = 1

  case result {
    Done(value) if keep -> value
    Failed(reason) -> reason
    Done(_) -> first
  }
}

fn first(pair: #(Int, Int)) -> Int {
  case pair {
    #(left, _) -> left
  }
}
