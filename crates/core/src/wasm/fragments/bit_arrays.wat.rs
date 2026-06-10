//! Checked WAT fragments for bit arrays runtime helpers.

pub(crate) const BIT_ARRAY_HELPERS: &str = r#"
  (func $__bit_array_payload_len (param $bit_len i32) (result i32)
    local.get $bit_len
    i32.const 7
    i32.add
    i32.const 3
    i32.shr_u
  )
  (func $__bit_array_new (param $data i32) (param $bit_len i32) (result i32)
    (local $ptr i32) (local $payload_len i32)
    local.get $bit_len
    call $__bit_array_payload_len
    local.set $payload_len
    local.get $payload_len
    i32.const 7
    i32.add
    i32.const -8
    i32.and
    i32.const 8
    i32.add
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 7
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $bit_len
    i32.store
    local.get $data
    local.get $ptr
    i32.const 8
    i32.add
    local.get $payload_len
    call $__copy_bytes
    local.get $ptr
  )
  (func $__bit_array_len (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 4
    i32.add
    i32.load
  )
  (func $__bit_array_data (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 8
    i32.add
  )
  (func $__bit_array_get_bit (param $ptr i32) (param $index i32) (result i32)
    local.get $index
    local.get $ptr
    call $__bit_array_len
    i32.ge_u
    if
      call $__panic
    end
    local.get $ptr
    call $__bit_array_data
    local.get $index
    i32.const 3
    i32.shr_u
    i32.add
    i32.load8_u
    i32.const 7
    local.get $index
    i32.const 7
    i32.and
    i32.sub
    i32.shr_u
    i32.const 1
    i32.and
  )
  (func $__bit_array_set_bit (param $data i32) (param $index i32) (param $bit i32)
    local.get $bit
    i32.const 1
    i32.and
    if
      local.get $data
      local.get $index
      i32.const 3
      i32.shr_u
      i32.add
      local.get $data
      local.get $index
      i32.const 3
      i32.shr_u
      i32.add
      i32.load8_u
      i32.const 1
      i32.const 7
      local.get $index
      i32.const 7
      i32.and
      i32.sub
      i32.shl
      i32.or
      i32.store8
    end
  )
  (func $__bit_array_slice (param $ptr i32) (param $start i32) (param $bit_len i32) (result i32)
    (local $out i32) (local $i i32)
    local.get $start
    local.get $bit_len
    i32.add
    local.get $ptr
    call $__bit_array_len
    i32.gt_u
    if
      call $__panic
    end
    i32.const 0
    local.get $bit_len
    call $__bit_array_new
    local.set $out
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $bit_len
        i32.ge_u
        br_if $done
        local.get $out
        call $__bit_array_data
        local.get $i
        local.get $ptr
        local.get $start
        local.get $i
        i32.add
        call $__bit_array_get_bit
        call $__bit_array_set_bit
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    local.get $out
  )
  (func $__bit_array_append (param $left i32) (param $right i32) (result i32)
    (local $out i32) (local $i i32) (local $left_len i32) (local $right_len i32)
    local.get $left
    call $__bit_array_len
    local.set $left_len
    local.get $right
    call $__bit_array_len
    local.set $right_len
    i32.const 0
    local.get $left_len
    local.get $right_len
    i32.add
    call $__bit_array_new
    local.set $out
    i32.const 0
    local.set $i
    block $done_left
      loop $left_loop
        local.get $i
        local.get $left_len
        i32.ge_u
        br_if $done_left
        local.get $out
        call $__bit_array_data
        local.get $i
        local.get $left
        local.get $i
        call $__bit_array_get_bit
        call $__bit_array_set_bit
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $left_loop
      end
    end
    i32.const 0
    local.set $i
    block $done_right
      loop $right_loop
        local.get $i
        local.get $right_len
        i32.ge_u
        br_if $done_right
        local.get $out
        call $__bit_array_data
        local.get $left_len
        local.get $i
        i32.add
        local.get $right
        local.get $i
        call $__bit_array_get_bit
        call $__bit_array_set_bit
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $right_loop
      end
    end
    local.get $out
  )
  (func $__bit_array_concat_list (param $list i32) (result i32)
    (local $result i32)
    i32.const 0
    i32.const 0
    call $__bit_array_new
    local.set $result
    block $done
      loop $loop
        local.get $list
        i32.eqz
        br_if $done
        local.get $result
        local.get $list
        call $__list_head
        i32.wrap_i64
        call $__bit_array_append
        local.set $result
        local.get $list
        call $__list_tail
        local.set $list
        br $loop
      end
    end
    local.get $result
  )
  (func $__bit_array_match (param $ptr i32) (param $start i32) (param $expected i32) (result i32)
    local.get $ptr
    local.get $start
    local.get $expected
    call $__bit_array_len
    call $__bit_array_slice
    local.get $expected
    call $__equal_value
  )
"#;
