//! Checked WAT fragments for debug runtime helpers.

pub(crate) const DEBUG_HELPERS: &str = r#"
  (func $__debug_tag (param $ptr i32) (result i32)
    local.get $ptr
    i32.eqz
    if (result i32)
      i32.const 0
    else
      local.get $ptr
      i32.load
    end
  )
  (func $__debug_reason (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 8
    i32.add
    i32.load
  )
  (func $__debug_payload_i64 (param $ptr i32) (param $index i32) (result i64)
    local.get $ptr
    i32.const 12
    i32.add
    local.get $index
    i32.const 8
    i32.mul
    i32.add
    i64.load
  )
"#;
