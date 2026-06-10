//! Checked WAT fragments for panic runtime helpers.

pub(crate) const PANIC_HELPERS: &str = r#"
  (func $__panic
    unreachable
  )
  (func $__assert (param $condition i32)
    local.get $condition
    i32.eqz
    if
      call $__panic
    end
  )
  (func $__match_fail
    call $__panic
  )
"#;
