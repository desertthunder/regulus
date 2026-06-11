pub type User {
  User(name: String, age: Int)
}

pub type Status {
  Ready(User)
  Empty
}

fn private_base() -> Int {
  1
}

pub fn new_user(age: Int) -> User {
  User(name: "Ada", age: age)
}

pub fn birthday(user: User) -> User {
  User(..user, age: age(user) + private_base())
}

pub fn age(user: User) -> Int {
  case user {
    User(name: _, age:) -> age
  }
}
