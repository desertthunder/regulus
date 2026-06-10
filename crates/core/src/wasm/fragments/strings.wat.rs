//! Checked WAT fragments for strings runtime helpers.

pub(crate) const STRING_HELPERS: &str = r#"
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
  (func $__string_concat_list (param $list i32) (result i32)
    (local $result i32)
    i32.const 0
    i32.const 0
    call $__string_new
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
        call $__string_concat
        local.set $result
        local.get $list
        call $__list_tail
        local.set $list
        br $loop
      end
    end
    local.get $result
  )
  (func $__int_to_string (param $value i64) (result i32)
    (local $n i64) (local $temp i64) (local $digits i32) (local $len i32) (local $ptr i32) (local $pos i32)
    local.get $value
    local.set $n
    local.get $value
    i64.const 0
    i64.lt_s
    if
      local.get $value
      i64.const -1
      i64.mul
      local.set $n
      i32.const 1
      local.set $len
    end
    local.get $n
    local.set $temp
    i32.const 1
    local.set $digits
    block $digits_done
      local.get $temp
      i64.const 10
      i64.lt_u
      br_if $digits_done
      i32.const 0
      local.set $digits
      loop $digits_loop
        local.get $temp
        i64.eqz
        br_if $digits_done
        local.get $temp
        i64.const 10
        i64.div_u
        local.set $temp
        local.get $digits
        i32.const 1
        i32.add
        local.set $digits
        br $digits_loop
      end
    end
    local.get $len
    local.get $digits
    i32.add
    local.set $len
    i32.const 0
    local.get $len
    call $__string_new
    local.set $ptr
    local.get $ptr
    call $__string_data
    local.get $len
    i32.add
    local.set $pos
    block $done
      loop $loop
        local.get $pos
        i32.const 1
        i32.sub
        local.set $pos
        local.get $pos
        local.get $n
        i64.const 10
        i64.rem_u
        i32.wrap_i64
        i32.const 48
        i32.add
        i32.store8
        local.get $n
        i64.const 10
        i64.div_u
        local.set $n
        local.get $n
        i64.eqz
        br_if $done
        br $loop
      end
    end
    local.get $value
    i64.const 0
    i64.lt_s
    if
      local.get $ptr
      call $__string_data
      i32.const 45
      i32.store8
    end
    local.get $ptr
  )
  (data (i32.const 1000) ".")
  (func $__float_to_string (param $value f64) (result i32)
    (local $int i64) (local $frac i64) (local $int_str i32) (local $dot i32) (local $frac_str i32)
    local.get $value
    i64.trunc_f64_s
    local.tee $int
    f64.convert_i64_s
    local.get $value
    f64.eq
    if (result i32)
      local.get $int
      call $__int_to_string
      return
    else
      i32.const 0
    end
    drop
    local.get $value
    local.get $int
    f64.convert_i64_s
    f64.sub
    local.tee $value
    f64.const 0
    f64.lt
    if
      f64.const -0
      local.get $value
      f64.sub
      local.set $value
    end
    local.get $value
    f64.const 1000000
    f64.mul
    i64.trunc_f64_u
    local.set $frac
    local.get $int
    call $__int_to_string
    local.set $int_str
    i32.const 1000
    i32.const 1
    call $__string_new
    local.set $dot
    local.get $frac
    call $__int_to_string
    local.set $frac_str
    local.get $int_str
    local.get $dot
    call $__string_concat
    local.get $frac_str
    call $__string_concat
  )
"#;
