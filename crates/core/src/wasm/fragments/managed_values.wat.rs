//! Checked WAT fragments for managed values runtime helpers.

pub(crate) const MANAGED_VALUE_HELPERS: &str = r#"
  (func $__tuple_new (param $arity i32) (param $fields i32) (result i32)
    (local $ptr i32)
    local.get $arity
    i32.const 8
    i32.mul
    i32.const 8
    i32.add
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 3
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $arity
    i32.store
    local.get $fields
    local.get $ptr
    i32.const 8
    i32.add
    local.get $arity
    call $__copy_slots
    local.get $ptr
  )
  (func $__record_new (param $arity i32) (param $fields i32) (result i32)
    (local $ptr i32)
    local.get $arity
    i32.const 8
    i32.mul
    i32.const 8
    i32.add
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 4
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $arity
    i32.store
    local.get $fields
    local.get $ptr
    i32.const 8
    i32.add
    local.get $arity
    call $__copy_slots
    local.get $ptr
  )
  (func $__custom_new (param $constructor i32) (param $arity i32) (param $fields i32) (result i32)
    (local $ptr i32)
    local.get $arity
    i32.const 8
    i32.mul
    i32.const 12
    i32.add
    i32.const 7
    i32.add
    i32.const -8
    i32.and
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 5
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $arity
    i32.store
    local.get $ptr
    i32.const 8
    i32.add
    local.get $constructor
    i32.store
    local.get $fields
    local.get $ptr
    i32.const 12
    i32.add
    local.get $arity
    call $__copy_slots
    local.get $ptr
  )
  (func $__field_load_i64 (param $ptr i32) (param $index i32) (result i64)
    local.get $ptr
    i32.const 8
    i32.add
    local.get $index
    i32.const 8
    i32.mul
    i32.add
    i64.load
  )
  (func $__closure_new (param $function_id i32) (param $capture_count i32) (param $captures i32) (result i32)
    (local $ptr i32)
    local.get $capture_count
    i32.const {closure_capture_slot_size}
    i32.mul
    i32.const {closure_captures_offset}
    i32.add
    i32.const 7
    i32.add
    i32.const -8
    i32.and
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 6
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $capture_count
    i32.store
    local.get $ptr
    i32.const {closure_function_id_offset}
    i32.add
    local.get $function_id
    i32.store
    local.get $captures
    local.get $ptr
    i32.const {closure_captures_offset}
    i32.add
    local.get $capture_count
    i32.const {closure_capture_slot_size}
    i32.mul
    call $__copy_bytes
    local.get $ptr
  )
  (func $__panic_value_new (param $reason i32) (param $arity i32) (param $fields i32) (result i32)
    (local $ptr i32)
    local.get $arity
    i32.const 8
    i32.mul
    i32.const 12
    i32.add
    i32.const 7
    i32.add
    i32.const -8
    i32.and
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 10
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $arity
    i32.store
    local.get $ptr
    i32.const 8
    i32.add
    local.get $reason
    i32.store
    local.get $fields
    local.get $ptr
    i32.const 12
    i32.add
    local.get $arity
    call $__copy_slots
    local.get $ptr
  )
  (func $__option_some (param $value i64) (result i32)
    (local $slots i32)
    i32.const 8
    call $__alloc
    local.set $slots
    local.get $slots
    local.get $value
    i64.store
    i32.const 2407843793
    i32.const 1
    local.get $slots
    call $__custom_new
  )
  (func $__option_none (result i32)
    i32.const 2443824955
    i32.const 0
    i32.const 0
    call $__custom_new
  )
  (func $__order_from_compare (param $compare i32) (result i32)
    local.get $compare
    i32.const 0
    i32.lt_s
    if (result i32)
      i32.const 1165421021
    else
      local.get $compare
      i32.const 0
      i32.gt_s
      if (result i32)
        i32.const 1249309180
      else
        i32.const 1282864351
      end
    end
    i32.const 0
    i32.const 0
    call $__custom_new
  )
"#;
