external fn load(key: String) -> String = "browser" "localStorage.getItem"

pub fn main(key: String) -> String {
  load(key)
}
