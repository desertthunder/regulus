//! Checked WAT fragments for dynamic values and primitive decoders.

pub(crate) const DYNAMIC_HELPERS: &str = r#"
  (func $__dynamic_i64 (param $tag i32) (param $value i64) (result i32)
    (local $slots i32)
    i32.const 8
    call $__alloc
    local.set $slots
    local.get $slots
    local.get $value
    i64.store
    local.get $tag
    i32.const 1
    local.get $slots
    call $__custom_new
  )
  (func $__dynamic_empty (param $tag i32) (result i32)
    local.get $tag
    i32.const 0
    i32.const 0
    call $__custom_new
  )
  (func $__dynamic_tag (param $value i32) (result i32)
    local.get $value
    i32.eqz
    if (result i32)
      i32.const 7
    else
      local.get $value
      i32.const 8
      i32.add
      i32.load
    end
  )
  (func $__dynamic_field0 (param $value i32) (result i64)
    local.get $value
    i32.const 12
    i32.add
    i64.load
  )
  (func $__dynamic_int (param $value i64) (result i32)
    i32.const 1
    local.get $value
    call $__dynamic_i64
  )
  (func $__dynamic_float (param $value f64) (result i32)
    i32.const 2
    local.get $value
    i64.reinterpret_f64
    call $__dynamic_i64
  )
  (func $__dynamic_bool (param $value i32) (result i32)
    i32.const 3
    local.get $value
    i64.extend_i32_u
    call $__dynamic_i64
  )
  (func $__dynamic_string (param $value i32) (result i32)
    i32.const 4
    local.get $value
    i64.extend_i32_u
    call $__dynamic_i64
  )
  (func $__dynamic_bit_array (param $value i32) (result i32)
    i32.const 5
    local.get $value
    i64.extend_i32_u
    call $__dynamic_i64
  )
  (func $__dynamic_list (param $value i32) (result i32)
    i32.const 6
    local.get $value
    i64.extend_i32_u
    call $__dynamic_i64
  )
  (func $__dynamic_nil (result i32)
    i32.const 7
    call $__dynamic_empty
  )
  (func $__dynamic_array (param $value i32) (result i32)
    i32.const 9
    local.get $value
    i64.extend_i32_u
    call $__dynamic_i64
  )
  (func $__dynamic_properties (param $entries i32) (result i32)
    (local $dict i32) (local $pair i32)
    call $__dict_new
    local.set $dict
    block $done
      loop $loop
        local.get $entries
        i32.eqz
        br_if $done
        local.get $entries
        call $__list_head
        i32.wrap_i64
        local.set $pair
        local.get $dict
        local.get $pair
        i32.const 0
        call $__field_load_i64
        local.get $pair
        i32.const 1
        call $__field_load_i64
        call $__dict_insert
        local.set $dict
        local.get $entries
        call $__list_tail
        local.set $entries
        br $loop
      end
    end
    i32.const 8
    local.get $dict
    i64.extend_i32_u
    call $__dynamic_i64
  )
  (func $__decoder (param $kind i32) (param $inner i32) (result i32)
    (local $slots i32)
    i32.const 16
    call $__alloc
    local.set $slots
    local.get $slots
    local.get $kind
    i64.extend_i32_u
    i64.store
    local.get $slots
    i32.const 8
    i32.add
    local.get $inner
    i64.extend_i32_u
    i64.store
    i32.const 200
    i32.const 2
    local.get $slots
    call $__custom_new
  )
  (func $__decoder_list (param $inner i32) (result i32)
    i32.const 106
    local.get $inner
    call $__decoder
  )
  (func $__decoder_optional (param $inner i32) (result i32)
    i32.const 107
    local.get $inner
    call $__decoder
  )
  (func $__decoder_kind (param $decoder i32) (result i32)
    local.get $decoder
    i32.const 12
    i32.add
    i64.load
    i32.wrap_i64
  )
  (func $__decode_ok (param $value i64) (result i32)
    (local $slots i32)
    i32.const 8
    call $__alloc
    local.set $slots
    local.get $slots
    local.get $value
    i64.store
    i32.const 1115088027
    i32.const 1
    local.get $slots
    call $__custom_new
  )
  (func $__decode_error (result i32)
    i32.const 4031082741
    i32.const 1
    i32.const 0
    call $__custom_new
  )
  (func $__decode_run (param $data i32) (param $decoder i32) (result i32)
    (local $kind i32) (local $tag i32) (local $field i64)
    local.get $decoder
    call $__decoder_kind
    local.set $kind
    local.get $data
    call $__dynamic_tag
    local.set $tag
    local.get $data
    i32.eqz
    if
      call $__decode_error
      return
    end
    local.get $data
    call $__dynamic_field0
    local.set $field
    local.get $kind
    i32.const 100
    i32.eq
    if
      local.get $data
      i64.extend_i32_u
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 101
    i32.eq
    local.get $tag
    i32.const 1
    i32.eq
    i32.and
    if
      local.get $field
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 102
    i32.eq
    local.get $tag
    i32.const 2
    i32.eq
    i32.and
    if
      local.get $field
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 103
    i32.eq
    local.get $tag
    i32.const 3
    i32.eq
    i32.and
    if
      local.get $field
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 104
    i32.eq
    local.get $tag
    i32.const 4
    i32.eq
    i32.and
    if
      local.get $field
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 105
    i32.eq
    local.get $tag
    i32.const 5
    i32.eq
    i32.and
    if
      local.get $field
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 106
    i32.eq
    local.get $tag
    i32.const 6
    i32.eq
    i32.and
    if
      local.get $field
      call $__decode_ok
      return
    end
    local.get $kind
    i32.const 107
    i32.eq
    if
      local.get $tag
      i32.const 7
      i32.eq
      if
        call $__option_none
        i64.extend_i32_u
        call $__decode_ok
        return
      end
      ;; This minimal decoder returns Some(original dynamic) for present values.
      local.get $data
      i64.extend_i32_u
      call $__option_some
      i64.extend_i32_u
      call $__decode_ok
      return
    end
    call $__decode_error
  )
"#;
