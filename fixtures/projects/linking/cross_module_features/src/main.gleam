import domain

pub fn run() -> Int {
  let user = domain.birthday(domain.new_user(41))
  case domain.Ready(user) {
    domain.Ready(domain.User(_, years)) -> years
    domain.Empty -> 0
  }
}
