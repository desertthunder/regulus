//! Checked WAT fragments for strings runtime helpers.

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
  (func $__string_starts_with (param $string i32) (param $prefix i32) (result i32)
    (local $string_len i32) (local $prefix_len i32) (local $i i32)
    local.get $string
    call $__string_len
    local.set $string_len
    local.get $prefix
    call $__string_len
    local.set $prefix_len
    ;; If prefix is longer than string, it cannot start with it.
    local.get $prefix_len
    local.get $string_len
    i32.gt_u
    if (result i32)
      i32.const 0
    else
      i32.const 1
      local.set $i
      block $match
        loop $loop
          local.get $i
          local.get $prefix_len
          i32.ge_u
          br_if $match
          local.get $string
          call $__string_data
          local.get $i
          i32.add
          i32.load8_u
          local.get $prefix
          call $__string_data
          local.get $i
          i32.add
          i32.load8_u
          i32.ne
          if
            i32.const 0
            return
          end
          local.get $i
          i32.const 1
          i32.add
          local.set $i
          br $loop
        end
      end
      i32.const 1
    end
  )
  (func $__string_ends_with (param $string i32) (param $suffix i32) (result i32)
    (local $string_len i32) (local $suffix_len i32) (local $offset i32) (local $i i32)
    local.get $string
    call $__string_len
    local.set $string_len
    local.get $suffix
    call $__string_len
    local.set $suffix_len
    ;; If suffix is longer than string, it cannot end with it.
    local.get $suffix_len
    local.get $string_len
    i32.gt_u
    if (result i32)
      i32.const 0
    else
      local.get $string_len
      local.get $suffix_len
      i32.sub
      local.set $offset
      i32.const 0
      local.set $i
      block $match
        loop $loop
          local.get $i
          local.get $suffix_len
          i32.ge_u
          br_if $match
          local.get $string
          call $__string_data
          local.get $offset
          local.get $i
          i32.add
          i32.add
          i32.load8_u
          local.get $suffix
          call $__string_data
          local.get $i
          i32.add
          i32.load8_u
          i32.ne
          if
            i32.const 0
            return
          end
          local.get $i
          i32.const 1
          i32.add
          local.set $i
          br $loop
        end
      end
      i32.const 1
    end
  )
  (func $__string_contains (param $haystack i32) (param $needle i32) (result i32)
    (local $haystack_len i32) (local $needle_len i32) (local $start i32) (local $j i32) (local $matched i32)
    local.get $haystack
    call $__string_len
    local.set $haystack_len
    local.get $needle
    call $__string_len
    local.set $needle_len
    ;; Empty needle is always found.
    local.get $needle_len
    i32.eqz
    if (result i32)
      i32.const 1
    else
      i32.const 0
      local.set $start
      block $not_found
        loop $start_loop
          local.get $start
          local.get $needle_len
          i32.add
          local.get $haystack_len
          i32.gt_u
          br_if $not_found
          i32.const 0
          local.set $j
          i32.const 1
          local.set $matched
          block $mismatch
            loop $match_loop
              local.get $j
              local.get $needle_len
              i32.ge_u
              br_if $mismatch
              local.get $haystack
              call $__string_data
              local.get $start
              local.get $j
              i32.add
              i32.add
              i32.load8_u
              local.get $needle
              call $__string_data
              local.get $j
              i32.add
              i32.load8_u
              i32.ne
              if
                i32.const 0
                local.set $matched
                br $mismatch
              end
              local.get $j
              i32.const 1
              i32.add
              local.set $j
              br $match_loop
            end
          end
          local.get $matched
          if
            i32.const 1
            return
          end
          local.get $start
          i32.const 1
          i32.add
          local.set $start
          br $start_loop
        end
      end
      i32.const 0
    end
  )
  ;; Percent encode: replaces unreserved characters except A-Za-z0-9-._~ with %XX.
  (func $__percent_encode (param $str i32) (result i32)
    (local $len i32) (local $data i32) (local $i i32) (local $byte i32) (local $out_len i32) (local $out i32) (local $out_data i32) (local $safe i32)
    local.get $str
    call $__string_len
    local.set $len
    local.get $str
    call $__string_data
    local.set $data
    ;; First pass: count output length.
    i32.const 0
    local.set $i
    i32.const 0
    local.set $out_len
    block $count_done
      loop $count_loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $count_done
        local.get $data
        local.get $i
        i32.add
        i32.load8_u
        local.set $byte
        ;; Check if character is unreserved (A-Za-z0-9-._~).
        i32.const 0
        local.set $safe
        local.get $byte
        i32.const 48
        i32.ge_u
        local.get $byte
        i32.const 57
        i32.le_u
        i32.and
        if
          i32.const 1
          local.set $safe
        end
        local.get $byte
        i32.const 65
        i32.ge_u
        local.get $byte
        i32.const 90
        i32.le_u
        i32.and
        if
          i32.const 1
          local.set $safe
        end
        local.get $byte
        i32.const 97
        i32.ge_u
        local.get $byte
        i32.const 122
        i32.le_u
        i32.and
        if
          i32.const 1
          local.set $safe
        end
        local.get $byte
        i32.const 45
        i32.eq
        local.get $byte
        i32.const 46
        i32.eq
        i32.or
        local.get $byte
        i32.const 95
        i32.eq
        i32.or
        local.get $byte
        i32.const 126
        i32.eq
        i32.or
        if
          i32.const 1
          local.set $safe
        end
        local.get $safe
        if (result i32)
          i32.const 1
        else
          i32.const 3
        end
        local.get $out_len
        i32.add
        local.set $out_len
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $count_loop
      end
    end
    ;; Allocate output string.
    i32.const 0
    local.get $out_len
    call $__string_new
    local.set $out
    local.get $out
    call $__string_data
    local.set $out_data
    ;; Second pass: encode.
    i32.const 0
    local.set $i
    i32.const 0
    local.set $out_len
    block $encode_done
      loop $encode_loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $encode_done
        local.get $data
        local.get $i
        i32.add
        i32.load8_u
        local.set $byte
        ;; Check if character is unreserved.
        i32.const 0
        local.set $safe
        local.get $byte
        i32.const 48
        i32.ge_u
        local.get $byte
        i32.const 57
        i32.le_u
        i32.and
        if
          i32.const 1
          local.set $safe
        end
        local.get $byte
        i32.const 65
        i32.ge_u
        local.get $byte
        i32.const 90
        i32.le_u
        i32.and
        if
          i32.const 1
          local.set $safe
        end
        local.get $byte
        i32.const 97
        i32.ge_u
        local.get $byte
        i32.const 122
        i32.le_u
        i32.and
        if
          i32.const 1
          local.set $safe
        end
        local.get $byte
        i32.const 45
        i32.eq
        local.get $byte
        i32.const 46
        i32.eq
        i32.or
        local.get $byte
        i32.const 95
        i32.eq
        i32.or
        local.get $byte
        i32.const 126
        i32.eq
        i32.or
        if
          i32.const 1
          local.set $safe
        end
        local.get $safe
        if
          ;; Copy byte as-is.
          local.get $out_data
          local.get $out_len
          i32.add
          local.get $byte
          i32.store8
          local.get $out_len
          i32.const 1
          i32.add
          local.set $out_len
        else
          ;; Write %XX.
          local.get $out_data
          local.get $out_len
          i32.add
          i32.const 37
          i32.store8
          local.get $out_data
          local.get $out_len
          i32.const 1
          i32.add
          i32.add
          local.get $byte
          i32.const 4
          i32.shr_u
          i32.const 15
          i32.and
          call $__nibble_to_hex
          i32.store8
          local.get $out_data
          local.get $out_len
          i32.const 2
          i32.add
          i32.add
          local.get $byte
          i32.const 15
          i32.and
          call $__nibble_to_hex
          i32.store8
          local.get $out_len
          i32.const 3
          i32.add
          local.set $out_len
        end
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $encode_loop
      end
    end
    local.get $out
  )
  ;; Percent decode: replaces %XX sequences with raw bytes. Returns 0 on error.
  (func $__percent_decode (param $str i32) (result i32)
    (local $len i32) (local $data i32) (local $i i32) (local $out_len i32) (local $out i32) (local $out_data i32) (local $byte i32) (local $hi i32) (local $lo i32)
    local.get $str
    call $__string_len
    local.set $len
    local.get $str
    call $__string_data
    local.set $data
    ;; First pass: count output length.
    i32.const 0
    local.set $i
    i32.const 0
    local.set $out_len
    block $count_done
      loop $count_loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $count_done
        local.get $data
        local.get $i
        i32.add
        i32.load8_u
        local.set $byte
        local.get $byte
        i32.const 37
        i32.eq
        if
          ;; Need at least 2 more chars.
          local.get $i
          i32.const 2
          i32.add
          local.get $len
          i32.ge_u
          if
            i32.const 0
            return
          end
          ;; Validate hex digits.
          local.get $data
          local.get $i
          i32.const 1
          i32.add
          i32.add
          i32.load8_u
          call $__hex_digit_value
          i32.const 255
          i32.eq
          if
            i32.const 0
            return
          end
          local.get $data
          local.get $i
          i32.const 2
          i32.add
          i32.add
          i32.load8_u
          call $__hex_digit_value
          i32.const 255
          i32.eq
          if
            i32.const 0
            return
          end
          local.get $out_len
          i32.const 1
          i32.add
          local.set $out_len
          local.get $i
          i32.const 3
          i32.add
          local.set $i
        else
          local.get $out_len
          i32.const 1
          i32.add
          local.set $out_len
          local.get $i
          i32.const 1
          i32.add
          local.set $i
        end
        br $count_loop
      end
    end
    ;; Allocate output string.
    i32.const 0
    local.get $out_len
    call $__string_new
    local.set $out
    local.get $out
    call $__string_data
    local.set $out_data
    ;; Second pass: decode.
    i32.const 0
    local.set $i
    i32.const 0
    local.set $out_len
    block $decode_done
      loop $decode_loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $decode_done
        local.get $data
        local.get $i
        i32.add
        i32.load8_u
        local.set $byte
        local.get $byte
        i32.const 37
        i32.eq
        if
          local.get $data
          local.get $i
          i32.const 1
          i32.add
          i32.add
          i32.load8_u
          call $__hex_digit_value
          local.set $hi
          local.get $data
          local.get $i
          i32.const 2
          i32.add
          i32.add
          i32.load8_u
          call $__hex_digit_value
          local.set $lo
          local.get $out_data
          local.get $out_len
          i32.add
          local.get $hi
          i32.const 4
          i32.shl
          local.get $lo
          i32.or
          i32.store8
          local.get $out_len
          i32.const 1
          i32.add
          local.set $out_len
          local.get $i
          i32.const 3
          i32.add
          local.set $i
        else
          local.get $out_data
          local.get $out_len
          i32.add
          local.get $byte
          i32.store8
          local.get $out_len
          i32.const 1
          i32.add
          local.set $out_len
          local.get $i
          i32.const 1
          i32.add
          local.set $i
        end
        br $decode_loop
      end
    end
    local.get $out
  )
"#;
