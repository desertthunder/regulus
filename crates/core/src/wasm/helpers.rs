//! WAT snippets for the generated runtime prelude.

pub const ALLOC_HELPER: &str = r#"
  (func $__last_panic (export "__last_panic") (result i32)
    global.get $__last_panic_payload
  )
  (func $__allocation_fail (param $size i32) (param $heap i32) (result i32)
    i32.const {allocation_failure_offset}
    i32.const 10
    i32.store
    i32.const {allocation_failure_offset}
    i32.const 4
    i32.add
    i32.const 2
    i32.store
    i32.const {allocation_failure_offset}
    i32.const 8
    i32.add
    i32.const 1
    i32.store
    i32.const {allocation_failure_offset}
    i32.const 12
    i32.add
    local.get $size
    i64.extend_i32_u
    i64.store
    i32.const {allocation_failure_offset}
    i32.const 20
    i32.add
    local.get $heap
    i64.extend_i32_u
    i64.store
    i32.const {allocation_failure_offset}
    global.set $__last_panic_payload
    unreachable
  )
  (func $__alloc (param $size i32) (result i32)
    (local $ptr i32) (local $end i32) (local $pages i32)
    global.get $__heap
    local.set $ptr
    global.get $__heap
    local.get $size
    i32.add
    i32.const {alignment_mask}
    i32.add
    i32.const -{alignment}
    i32.and
    local.set $end
    local.get $end
    local.get $ptr
    i32.lt_u
    if (result i32)
      local.get $size
      local.get $ptr
      call $__allocation_fail
      return
    else
      i32.const 0
    end
    drop
    local.get $end
    memory.size
    i32.const 65536
    i32.mul
    i32.gt_u
    if
      local.get $end
      memory.size
      i32.const 65536
      i32.mul
      i32.sub
      i32.const 65535
      i32.add
      i32.const 16
      i32.shr_u
      local.tee $pages
      memory.grow
      i32.const -1
      i32.eq
      if (result i32)
        local.get $size
        local.get $ptr
        call $__allocation_fail
        return
      else
        i32.const 0
      end
      drop
    end
    local.get $end
    global.set $__heap
    local.get $ptr
  )
"#;

pub const PANIC_HELPERS: &str = r#"
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

pub const COPY_HELPERS: &str = r#"
  (func $__copy_bytes (param $src i32) (param $dst i32) (param $len i32)
    (local $i i32)
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $done
        local.get $dst
        local.get $i
        i32.add
        local.get $src
        local.get $i
        i32.add
        i32.load8_u
        i32.store8
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
  )
  (func $__copy_slots (param $src i32) (param $dst i32) (param $count i32)
    (local $i i32)
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $count
        i32.ge_u
        br_if $done
        local.get $dst
        local.get $i
        i32.const 8
        i32.mul
        i32.add
        local.get $src
        local.get $i
        i32.const 8
        i32.mul
        i32.add
        i64.load
        i64.store
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
  )
"#;

pub const STRING_HELPERS: &str = r#"
  (func $__string_new (param $data i32) (param $len i32) (result i32)
    (local $ptr i32)
    local.get $len
    i32.const 7
    i32.add
    i32.const -8
    i32.and
    i32.const 8
    i32.add
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 1
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    local.get $len
    i32.store
    local.get $data
    local.get $ptr
    i32.const 8
    i32.add
    local.get $len
    call $__copy_bytes
    local.get $ptr
  )
  (func $__string_len (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 4
    i32.add
    i32.load
  )
  (func $__string_data (param $ptr i32) (result i32)
    local.get $ptr
    i32.const 8
    i32.add
  )
  (func $__string_concat (param $left i32) (param $right i32) (result i32)
    (local $ptr i32) (local $left_len i32) (local $right_len i32)
    local.get $left
    call $__string_len
    local.set $left_len
    local.get $right
    call $__string_len
    local.set $right_len
    i32.const 0
    local.get $left_len
    local.get $right_len
    i32.add
    call $__string_new
    local.set $ptr
    local.get $left
    call $__string_data
    local.get $ptr
    call $__string_data
    local.get $left_len
    call $__copy_bytes
    local.get $right
    call $__string_data
    local.get $ptr
    call $__string_data
    local.get $left_len
    i32.add
    local.get $right_len
    call $__copy_bytes
    local.get $ptr
  )
  (func $__string_compare (param $left i32) (param $right i32) (result i32)
    (local $i i32) (local $left_len i32) (local $right_len i32) (local $lb i32) (local $rb i32)
    local.get $left
    call $__string_len
    local.set $left_len
    local.get $right
    call $__string_len
    local.set $right_len
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $left_len
        i32.ge_u
        local.get $i
        local.get $right_len
        i32.ge_u
        i32.or
        br_if $done
        local.get $left
        call $__string_data
        local.get $i
        i32.add
        i32.load8_u
        local.set $lb
        local.get $right
        call $__string_data
        local.get $i
        i32.add
        i32.load8_u
        local.set $rb
        local.get $lb
        local.get $rb
        i32.ne
        if (result i32)
          local.get $lb
          local.get $rb
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
    local.get $left_len
    local.get $right_len
    i32.eq
    if (result i32)
      i32.const 0
    else
      local.get $left_len
      local.get $right_len
      i32.lt_u
      if (result i32)
        i32.const -1
      else
        i32.const 1
      end
    end
  )
  (func $__string_inspect (param $ptr i32) (result i32)
    local.get $ptr
  )
"#;

pub const BIT_ARRAY_HELPERS: &str = r#"
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

pub const EQUALITY_AND_ORDERING_HELPERS: &str = r#"
  (func $__equal_ptr (param $left i32) (param $right i32) (result i32)
    local.get $left
    local.get $right
    i32.eq
  )
  (func $__equal_i64 (param $left i64) (param $right i64) (result i32)
    local.get $left
    local.get $right
    i64.eq
  )
  (func $__equal_f64 (param $left f64) (param $right f64) (result i32)
    local.get $left
    local.get $right
    f64.eq
  )
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
  (func $__compare_bool (param $left i32) (param $right i32) (result i32)
    local.get $left
    local.get $right
    i32.sub
  )
  (func $__compare_string (param $left i32) (param $right i32) (result i32)
    local.get $left
    local.get $right
    call $__string_compare
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

pub const DEBUG_HELPERS: &str = r#"
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
  (func $__debug_size (param $ptr i32) (result i32)
    local.get $ptr
    i32.eqz
    if (result i32)
      i32.const 0
    else
      local.get $ptr
      i32.const 4
      i32.add
      i32.load
    end
  )
  (func $__debug_inspect (param $ptr i32) (result i32)
    local.get $ptr
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
  (func $__debug_payload_i32 (param $ptr i32) (param $index i32) (result i32)
    local.get $ptr
    local.get $index
    call $__debug_payload_i64
    i32.wrap_i64
  )
"#;

pub const MANAGED_VALUE_HELPERS: &str = r#"
  (func $__list_cons (param $head i64) (param $tail i32) (result i32)
    (local $ptr i32)
    i32.const 24
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 2
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    i32.const 2
    i32.store
    local.get $ptr
    i32.const 8
    i32.add
    local.get $head
    i64.store
    local.get $ptr
    i32.const 16
    i32.add
    local.get $tail
    i32.store
    local.get $ptr
  )
  (func $__list_head (param $ptr i32) (result i64)
    local.get $ptr
    i32.eqz
    if
      call $__panic
    end
    local.get $ptr
    i32.const 8
    i32.add
    i64.load
  )
  (func $__list_tail (param $ptr i32) (result i32)
    local.get $ptr
    i32.eqz
    if
      call $__panic
    end
    local.get $ptr
    i32.const 16
    i32.add
    i32.load
  )
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
  (func $__field_load_i32 (param $ptr i32) (param $index i32) (result i32)
    local.get $ptr
    local.get $index
    call $__field_load_i64
    i32.wrap_i64
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
  (func $__opaque_new (param $type_tag i32) (param $payload i32) (result i32)
    (local $ptr i32)
    i32.const 16
    call $__alloc
    local.set $ptr
    local.get $ptr
    i32.const 8
    i32.store
    local.get $ptr
    i32.const 4
    i32.add
    i32.const 0
    i32.store
    local.get $ptr
    i32.const 8
    i32.add
    local.get $type_tag
    i32.store
    local.get $ptr
    i32.const 12
    i32.add
    local.get $payload
    i32.store
    local.get $ptr
  )
  (func $__error_new (param $reason i32) (param $arity i32) (param $fields i32) (result i32)
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
    i32.const 9
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
"#;
