//! Checked WAT fragments for equality ordering runtime helpers.

pub const EQUALITY_AND_ORDERING_HELPERS: &str = r#"
  (func $__equal_bytes (param $left i32) (param $right i32) (param $len i32) (result i32)
    (local $i i32)
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $done
        local.get $left
        local.get $i
        i32.add
        i32.load8_u
        local.get $right
        local.get $i
        i32.add
        i32.load8_u
        i32.ne
        if (result i32)
          i32.const 0
          return
        else
          i32.const 0
        end
        drop
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    i32.const 1
  )
  (func $__is_managed_ptr (param $ptr i32) (result i32)
    (local $tag i32)
    local.get $ptr
    i32.eqz
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $ptr
    i32.const 7
    i32.and
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $ptr
    i32.const 8
    i32.add
    memory.size
    i32.const 65536
    i32.mul
    i32.gt_u
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $ptr
    i32.load
    local.tee $tag
    i32.const 1
    i32.ge_u
    local.get $tag
    i32.const 10
    i32.le_u
    i32.and
  )
  (func $__equal_slot (param $left i64) (param $right i64) (result i32)
    (local $left_ptr i32) (local $right_ptr i32)
    local.get $left
    local.get $right
    i64.eq
    if (result i32)
      i32.const 1
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i64.const 4294967295
    i64.gt_u
    local.get $right
    i64.const 4294967295
    i64.gt_u
    i32.or
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.wrap_i64
    local.tee $left_ptr
    call $__is_managed_ptr
    local.get $right
    i32.wrap_i64
    local.tee $right_ptr
    call $__is_managed_ptr
    i32.and
    if (result i32)
      local.get $left_ptr
      local.get $right_ptr
      call $__equal_value
    else
      i32.const 0
    end
  )
  (func $__equal_value (param $left i32) (param $right i32) (result i32)
    (local $tag i32) (local $len i32)
    local.get $left
    local.get $right
    i32.eq
    if (result i32)
      i32.const 1
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.eqz
    local.get $right
    i32.eqz
    i32.or
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.load
    local.tee $tag
    local.get $right
    i32.load
    i32.ne
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.const 4
    i32.add
    i32.load
    local.tee $len
    local.get $right
    i32.const 4
    i32.add
    i32.load
    i32.ne
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $tag
    i32.const 1
    i32.eq
    if (result i32)
      local.get $left
      call $__string_data
      local.get $right
      call $__string_data
      local.get $len
      call $__equal_bytes
      return
    else
      i32.const 0
    end
    drop
    local.get $tag
    i32.const 7
    i32.eq
    if (result i32)
      local.get $left
      call $__bit_array_data
      local.get $right
      call $__bit_array_data
      local.get $len
      call $__bit_array_payload_len
      call $__equal_bytes
      return
    else
      i32.const 0
    end
    drop
    local.get $tag
    i32.const 5
    i32.eq
    if (result i32)
      local.get $left
      i32.const 8
      i32.add
      local.get $right
      i32.const 8
      i32.add
      local.get $len
      i32.const 1
      i32.add
      call $__equal_slots
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.const 8
    i32.add
    local.get $right
    i32.const 8
    i32.add
    local.get $len
    call $__equal_slots
  )
  (func $__equal_slots (param $left i32) (param $right i32) (param $count i32) (result i32)
    (local $i i32)
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $count
        i32.ge_u
        br_if $done
        local.get $left
        local.get $i
        i32.const 8
        i32.mul
        i32.add
        i64.load
        local.get $right
        local.get $i
        i32.const 8
        i32.mul
        i32.add
        i64.load
        call $__equal_slot
        i32.eqz
        if (result i32)
          i32.const 0
          return
        else
          i32.const 0
        end
        drop
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    i32.const 1
  )
  (func $__compare_i64 (param $left i64) (param $right i64) (result i32)
    local.get $left
    local.get $right
    i64.eq
    if (result i32)
      i32.const 0
    else
      local.get $left
      local.get $right
      i64.lt_s
      if (result i32)
        i32.const -1
      else
        i32.const 1
      end
    end
  )
  (func $__compare_f64 (param $left f64) (param $right f64) (result i32)
    local.get $left
    local.get $right
    f64.eq
    if (result i32)
      i32.const 0
    else
      local.get $left
      local.get $right
      f64.lt
      if (result i32)
        i32.const -1
      else
        i32.const 1
      end
    end
  )
  (func $__compare_bytes (param $left i32) (param $right i32) (param $len i32) (result i32)
    (local $i i32) (local $left_byte i32) (local $right_byte i32)
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $done
        local.get $left
        local.get $i
        i32.add
        i32.load8_u
        local.set $left_byte
        local.get $right
        local.get $i
        i32.add
        i32.load8_u
        local.set $right_byte
        local.get $left_byte
        local.get $right_byte
        i32.ne
        if (result i32)
          local.get $left_byte
          local.get $right_byte
          i32.lt_u
          if (result i32)
            i32.const -1
          else
            i32.const 1
          end
          return
        else
          i32.const 0
        end
        drop
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    i32.const 0
  )
  (func $__compare_slot (param $left i64) (param $right i64) (result i32)
    (local $left_ptr i32) (local $right_ptr i32)
    local.get $left
    i64.const 4294967295
    i64.le_u
    local.get $right
    i64.const 4294967295
    i64.le_u
    i32.and
    if (result i32)
      local.get $left
      i32.wrap_i64
      local.tee $left_ptr
      call $__is_managed_ptr
      local.get $right
      i32.wrap_i64
      local.tee $right_ptr
      call $__is_managed_ptr
      i32.and
      if (result i32)
        local.get $left_ptr
        local.get $right_ptr
        call $__compare_value
        return
      else
        i32.const 0
      end
    else
      i32.const 0
    end
    drop
    local.get $left
    local.get $right
    call $__compare_i64
  )
  (func $__compare_slots (param $left i32) (param $right i32) (param $count i32) (result i32)
    (local $i i32) (local $result i32)
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $count
        i32.ge_u
        br_if $done
        local.get $left
        local.get $i
        i32.const 8
        i32.mul
        i32.add
        i64.load
        local.get $right
        local.get $i
        i32.const 8
        i32.mul
        i32.add
        i64.load
        call $__compare_slot
        local.tee $result
        if (result i32)
          local.get $result
          return
        else
          i32.const 0
        end
        drop
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    i32.const 0
  )
  (func $__compare_value (param $left i32) (param $right i32) (result i32)
    (local $tag i32) (local $len i32) (local $right_len i32) (local $byte_len i32) (local $result i32)
    local.get $left
    local.get $right
    i32.eq
    if (result i32)
      i32.const 0
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.eqz
    if (result i32)
      i32.const -1
      return
    else
      i32.const 0
    end
    drop
    local.get $right
    i32.eqz
    if (result i32)
      i32.const 1
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.load
    local.tee $tag
    i64.extend_i32_s
    local.get $right
    i32.load
    i64.extend_i32_s
    call $__compare_i64
    local.tee $result
    if (result i32)
      local.get $result
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.const 4
    i32.add
    i32.load
    local.set $len
    local.get $right
    i32.const 4
    i32.add
    i32.load
    local.set $right_len
    local.get $tag
    i32.const 1
    i32.eq
    if (result i32)
      local.get $left
      local.get $right
      call $__string_compare
      return
    else
      i32.const 0
    end
    drop
    local.get $tag
    i32.const 7
    i32.eq
    if (result i32)
      local.get $len
      call $__bit_array_payload_len
      local.get $right_len
      call $__bit_array_payload_len
      local.get $len
      local.get $right_len
      i32.lt_u
      select
      local.set $byte_len
      local.get $left
      call $__bit_array_data
      local.get $right
      call $__bit_array_data
      local.get $byte_len
      call $__compare_bytes
      local.tee $result
      if (result i32)
        local.get $result
        return
      else
        i32.const 0
      end
      drop
      local.get $len
      i64.extend_i32_s
      local.get $right_len
      i64.extend_i32_s
      call $__compare_i64
      return
    else
      i32.const 0
    end
    drop
    local.get $tag
    i32.const 6
    i32.eq
    local.get $tag
    i32.const 8
    i32.eq
    i32.or
    if (result i32)
      local.get $left
      i64.extend_i32_s
      local.get $right
      i64.extend_i32_s
      call $__compare_i64
      return
    else
      i32.const 0
    end
    drop
    local.get $tag
    i32.const 5
    i32.eq
    local.get $tag
    i32.const 9
    i32.eq
    i32.or
    local.get $tag
    i32.const 10
    i32.eq
    i32.or
    if (result i32)
      local.get $left
      i32.const 8
      i32.add
      local.get $right
      i32.const 8
      i32.add
      local.get $len
      i32.const 1
      i32.add
      call $__compare_slots
      return
    else
      i32.const 0
    end
    drop
    local.get $len
    i64.extend_i32_s
    local.get $right_len
    i64.extend_i32_s
    call $__compare_i64
    local.tee $result
    if (result i32)
      local.get $result
      return
    else
      i32.const 0
    end
    drop
    local.get $left
    i32.const 8
    i32.add
    local.get $right
    i32.const 8
    i32.add
    local.get $len
    call $__compare_slots
  )
"#;
