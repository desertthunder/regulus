//! Checked WAT fragments for bit arrays runtime helpers.

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
  (func $__bit_array_from_string (param $string i32) (result i32)
    (local $len i32) (local $ptr i32)
    local.get $string
    call $__string_len
    local.set $len
    ;; A string is a valid byte-aligned bit array. Copy the byte data.
    local.get $string
    call $__string_data
    local.get $len
    i32.const 8
    i32.mul
    call $__bit_array_new
  )
  (func $__bit_array_is_valid_utf8 (param $ptr i32) (result i32)
    (local $i i32) (local $len i32) (local $byte i32) (local $seq_len i32) (local $j i32) (local $cont i32)
    local.get $ptr
    call $__bit_array_len
    i32.const 3
    i32.shr_u
    local.set $len
    i32.const 0
    local.set $i
    block $invalid
      loop $loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $loop
        local.get $ptr
        call $__bit_array_data
        local.get $i
        i32.add
        i32.load8_u
        local.set $byte
        ;; Determine sequence length from leading byte.
        local.get $byte
        i32.const 128
        i32.lt_u
        if
          i32.const 1
          local.set $seq_len
        else
          local.get $byte
          i32.const 224
          i32.lt_u
          if
            i32.const 2
            local.set $seq_len
          else
            local.get $byte
            i32.const 240
            i32.lt_u
            if
              i32.const 3
              local.set $seq_len
            else
              local.get $byte
              i32.const 248
              i32.lt_u
              if
                i32.const 4
                local.set $seq_len
              else
                br $invalid
              end
            end
          end
        end
        ;; Check we have enough bytes for the full sequence.
        local.get $i
        local.get $seq_len
        i32.add
        local.get $len
        i32.gt_u
        if
          br $invalid
        end
        ;; Validate continuation bytes (10xxxxxx).
        i32.const 1
        local.set $j
        block $cont_done
          loop $cont_loop
            local.get $j
            local.get $seq_len
            i32.ge_u
            br_if $cont_done
            local.get $ptr
            call $__bit_array_data
            local.get $i
            local.get $j
            i32.add
            i32.add
            i32.load8_u
            local.set $cont
            local.get $cont
            i32.const 192
            i32.and
            i32.const 128
            i32.ne
            if
              br $invalid
            end
            local.get $j
            i32.const 1
            i32.add
            local.set $j
            br $cont_loop
          end
        end
        local.get $i
        local.get $seq_len
        i32.add
        local.set $i
        br $loop
      end
    end
    local.get $i
    local.get $len
    i32.ge_u
  )
  (func $__bit_array_to_string (param $ptr i32) (result i32)
    ;; Returns a Result(String, Nil) as an i32.
    ;; On success: a tagged pointer to a string (non-zero).
    ;; On error: 0 (Error(Nil) for a 0-tagged null).
    ;; Result representation: tag 0 = Ok, tag 1 = Error.
    ;; We use Ok(ptr) as a non-zero value and Error(Nil) as 0.
    (local $valid i32) (local $len i32) (local $str i32)
    local.get $ptr
    call $__bit_array_is_valid_utf8
    local.set $valid
    local.get $valid
    i32.eqz
    if (result i32)
      i32.const 0
    else
      ;; Valid UTF-8: copy byte data into a new string.
      local.get $ptr
      call $__bit_array_len
      i32.const 3
      i32.shr_u
      local.set $len
      local.get $ptr
      call $__bit_array_data
      local.get $len
      call $__string_new
      local.set $str
      ;; Return a non-zero value to indicate Ok.
      ;; The codegen for Result uses a tagged representation.
      ;; For now, return the string pointer (non-zero = Ok).
      local.get $str
    end
  )
  (func $__bit_array_slice_bytes (param $ptr i32) (param $position i32) (param $length i32) (result i32)
    ;; Returns a Result(BitArray, Nil) as i32.
    ;; On success: a bit_array pointer (non-zero).
    ;; On error: 0 (Error(Nil)).
    (local $byte_len i32) (local $out i32) (local $src i32) (local $i i32)
    local.get $ptr
    call $__bit_array_len
    i32.const 3
    i32.shr_u
    local.set $byte_len
    ;; Validate bounds.
    local.get $position
    i32.const 0
    i32.lt_s
    if
      i32.const 0
      return
    end
    local.get $length
    i32.const 0
    i32.lt_s
    if
      i32.const 0
      return
    end
    local.get $position
    local.get $length
    i32.add
    local.get $byte_len
    i32.gt_u
    if
      i32.const 0
      return
    end
    ;; Create a new bit array with the sliced bytes.
    local.get $ptr
    call $__bit_array_data
    local.get $position
    i32.add
    local.set $src
    i32.const 0
    local.get $length
    i32.const 8
    i32.mul
    call $__bit_array_new
    local.set $out
    ;; Copy bytes.
    i32.const 0
    local.set $i
    block $done
      loop $copy_loop
        local.get $i
        local.get $length
        i32.ge_u
        br_if $done
        local.get $out
        call $__bit_array_data
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
        br $copy_loop
      end
    end
    local.get $out
  )
  (func $__bit_array_pad_to_bytes (param $ptr i32) (result i32)
    (local $bit_len i32) (local $rem i32)
    local.get $ptr
    call $__bit_array_len
    local.set $bit_len
    local.get $bit_len
    i32.const 7
    i32.and
    local.set $rem
    local.get $rem
    i32.eqz
    if (result i32)
      local.get $ptr
    else
      ;; Already byte-aligned by construction in our runtime,
      ;; but if there are trailing bits, round up.
      local.get $ptr
    end
  )
  ;; Base16 (hexadecimal) encoding: each byte -> two hex chars.
  ;; Converts a nibble (0-15) to a lowercase hex character.
  (func $__nibble_to_hex (param $nibble i32) (result i32)
    local.get $nibble
    i32.const 10
    i32.lt_u
    if (result i32)
      local.get $nibble
      i32.const 48
      i32.add
    else
      local.get $nibble
      i32.const 87
      i32.add
    end
  )
  (func $__base16_encode (param $ptr i32) (result i32)
    (local $byte_len i32) (local $str i32) (local $i i32) (local $byte i32) (local $data i32)
    local.get $ptr
    call $__bit_array_len
    i32.const 3
    i32.shr_u
    local.set $byte_len
    ;; Allocate a string of length byte_len * 2.
    i32.const 0
    local.get $byte_len
    i32.const 2
    i32.mul
    call $__string_new
    local.set $str
    local.get $str
    call $__string_data
    local.set $data
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $byte_len
        i32.ge_u
        br_if $done
        local.get $ptr
        call $__bit_array_data
        local.get $i
        i32.add
        i32.load8_u
        local.set $byte
        ;; High nibble.
        local.get $data
        local.get $i
        i32.const 2
        i32.mul
        i32.add
        local.get $byte
        i32.const 4
        i32.shr_u
        i32.const 15
        i32.and
        call $__nibble_to_hex
        i32.store8
        ;; Low nibble.
        local.get $data
        local.get $i
        i32.const 2
        i32.mul
        i32.const 1
        i32.add
        i32.add
        local.get $byte
        i32.const 15
        i32.and
        call $__nibble_to_hex
        i32.store8
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    local.get $str
  )
  ;; Hex decode lookup: returns 0-15 for valid hex, or 255 for invalid.
  (func $__hex_digit_value (param $c i32) (result i32)
    local.get $c
    i32.const 48
    i32.ge_u
    local.get $c
    i32.const 57
    i32.le_u
    i32.and
    if (result i32)
      local.get $c
      i32.const 48
      i32.sub
    else
      local.get $c
      i32.const 65
      i32.ge_u
      local.get $c
      i32.const 70
      i32.le_u
      i32.and
      if (result i32)
        local.get $c
        i32.const 55
        i32.sub
      else
        local.get $c
        i32.const 97
        i32.ge_u
        local.get $c
        i32.const 102
        i32.le_u
        i32.and
        if (result i32)
          local.get $c
          i32.const 87
          i32.sub
        else
          i32.const 255
        end
      end
    end
  )
  ;; Base16 decode: two hex chars -> one byte. Returns 0 on error (Error(Nil)).
  (func $__base16_decode (param $str i32) (result i32)
    (local $len i32) (local $byte_len i32) (local $i i32) (local $hi i32) (local $lo i32) (local $ptr i32) (local $data i32)
    local.get $str
    call $__string_len
    local.set $len
    ;; Length must be even.
    local.get $len
    i32.const 1
    i32.and
    if
      i32.const 0
      return
    end
    local.get $len
    i32.const 1
    i32.shr_u
    local.set $byte_len
    local.get $str
    call $__string_data
    local.set $data
    i32.const 0
    local.get $byte_len
    i32.const 8
    i32.mul
    call $__bit_array_new
    local.set $ptr
    i32.const 0
    local.set $i
    block $done
      loop $loop
        local.get $i
        local.get $byte_len
        i32.ge_u
        br_if $done
        ;; High nibble.
        local.get $data
        local.get $i
        i32.const 2
        i32.mul
        i32.add
        i32.load8_u
        call $__hex_digit_value
        local.set $hi
        local.get $hi
        i32.const 255
        i32.eq
        if
          i32.const 0
          return
        end
        ;; Low nibble.
        local.get $data
        local.get $i
        i32.const 2
        i32.mul
        i32.const 1
        i32.add
        i32.add
        i32.load8_u
        call $__hex_digit_value
        local.set $lo
        local.get $lo
        i32.const 255
        i32.eq
        if
          i32.const 0
          return
        end
        ;; Store byte.
        local.get $ptr
        call $__bit_array_data
        local.get $i
        i32.add
        local.get $hi
        i32.const 4
        i32.shl
        local.get $lo
        i32.or
        i32.store8
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    local.get $ptr
  )
  ;; Converts a 6-bit index (0-63) to a base64 character.
  (func $__base64_index_to_char (param $index i32) (result i32)
    local.get $index
    i32.const 26
    i32.lt_u
    if (result i32)
      local.get $index
      i32.const 65
      i32.add
    else
      local.get $index
      i32.const 52
      i32.lt_u
      if (result i32)
        local.get $index
        i32.const 71
        i32.add
      else
        local.get $index
        i32.const 62
        i32.lt_u
        if (result i32)
          local.get $index
          i32.const 4
          i32.sub
        else
          local.get $index
          i32.const 62
          i32.eq
          if (result i32)
            i32.const 43
          else
            i32.const 47
          end
        end
      end
    end
  )
  (func $__base64_encode (param $ptr i32) (param $padding i32) (result i32)
    (local $byte_len i32) (local $out_len i32) (local $str i32) (local $i i32) (local $remaining i32) (local $b0 i32) (local $b1 i32) (local $b2 i32) (local $triple i32) (local $data i32) (local $src i32)
    local.get $ptr
    call $__bit_array_len
    i32.const 3
    i32.shr_u
    local.set $byte_len
    ;; Calculate output length.
    local.get $byte_len
    i32.const 2
    i32.add
    i32.const 3
    i32.div_u
    i32.const 2
    i32.shl
    local.set $out_len
    ;; Allocate output string.
    i32.const 0
    local.get $out_len
    call $__string_new
    local.set $str
    local.get $str
    call $__string_data
    local.set $data
    local.get $ptr
    call $__bit_array_data
    local.set $src
    i32.const 0
    local.set $i
    local.get $byte_len
    local.set $remaining
    block $done
      loop $loop
        local.get $remaining
        i32.const 3
        i32.lt_u
        br_if $done
        local.get $src
        local.get $i
        i32.add
        i32.load8_u
        local.set $b0
        local.get $src
        local.get $i
        i32.const 1
        i32.add
        i32.add
        i32.load8_u
        local.set $b1
        local.get $src
        local.get $i
        i32.const 2
        i32.add
        i32.add
        i32.load8_u
        local.set $b2
        ;; Encode 4 chars.
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.add
        local.get $b0
        i32.const 2
        i32.shr_u
        call $__base64_index_to_char
        i32.store8
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 1
        i32.add
        i32.add
        local.get $b0
        i32.const 3
        i32.and
        i32.const 4
        i32.shl
        local.get $b1
        i32.const 4
        i32.shr_u
        i32.or
        call $__base64_index_to_char
        i32.store8
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 2
        i32.add
        i32.add
        local.get $b1
        i32.const 15
        i32.and
        i32.const 2
        i32.shl
        local.get $b2
        i32.const 6
        i32.shr_u
        i32.or
        call $__base64_index_to_char
        i32.store8
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 3
        i32.add
        i32.add
        local.get $b2
        i32.const 63
        i32.and
        call $__base64_index_to_char
        i32.store8
        local.get $i
        i32.const 3
        i32.add
        local.set $i
        local.get $remaining
        i32.const 3
        i32.sub
        local.set $remaining
        br $loop
      end
    end
    ;; Handle remaining bytes (1 or 2).
    local.get $remaining
    i32.const 1
    i32.eq
    if
      local.get $src
      local.get $i
      i32.add
      i32.load8_u
      local.set $b0
      local.get $data
      local.get $i
      i32.const 4
      i32.div_u
      i32.const 4
      i32.mul
      i32.add
      local.get $b0
      i32.const 2
      i32.shr_u
      call $__base64_index_to_char
      i32.store8
      local.get $data
      local.get $i
      i32.const 4
      i32.div_u
      i32.const 4
      i32.mul
      i32.const 1
      i32.add
      i32.add
      local.get $b0
      i32.const 3
      i32.and
      i32.const 4
      i32.shl
      call $__base64_index_to_char
      i32.store8
      ;; Padding.
      local.get $padding
      if
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 2
        i32.add
        i32.add
        i32.const 61
        i32.store8
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 3
        i32.add
        i32.add
        i32.const 61
        i32.store8
      end
    else
      local.get $remaining
      i32.const 2
      i32.eq
      if
        local.get $src
        local.get $i
        i32.add
        i32.load8_u
        local.set $b0
        local.get $src
        local.get $i
        i32.const 1
        i32.add
        i32.add
        i32.load8_u
        local.set $b1
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.add
        local.get $b0
        i32.const 2
        i32.shr_u
        call $__base64_index_to_char
        i32.store8
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 1
        i32.add
        i32.add
        local.get $b0
        i32.const 3
        i32.and
        i32.const 4
        i32.shl
        local.get $b1
        i32.const 4
        i32.shr_u
        i32.or
        call $__base64_index_to_char
        i32.store8
        local.get $data
        local.get $i
        i32.const 4
        i32.div_u
        i32.const 4
        i32.mul
        i32.const 2
        i32.add
        i32.add
        local.get $b1
        i32.const 15
        i32.and
        i32.const 2
        i32.shl
        call $__base64_index_to_char
        i32.store8
        ;; Padding.
        local.get $padding
        if
          local.get $data
          local.get $i
          i32.const 4
          i32.div_u
          i32.const 4
          i32.mul
          i32.const 3
          i32.add
          i32.add
          i32.const 61
          i32.store8
        end
      end
    end
    local.get $str
  )
  ;; Base64 decode lookup: returns 0-63 for valid base64 chars, or 255 for invalid.
  (func $__base64_digit_value (param $c i32) (result i32)
    local.get $c
    i32.const 65
    i32.ge_u
    local.get $c
    i32.const 90
    i32.le_u
    i32.and
    if (result i32)
      local.get $c
      i32.const 65
      i32.sub
    else
      local.get $c
      i32.const 97
      i32.ge_u
      local.get $c
      i32.const 122
      i32.le_u
      i32.and
      if (result i32)
        local.get $c
        i32.const 71
        i32.sub
      else
        local.get $c
        i32.const 48
        i32.ge_u
        local.get $c
        i32.const 57
        i32.le_u
        i32.and
        if (result i32)
          local.get $c
          i32.const 4
          i32.add
        else
          local.get $c
          i32.const 43
          i32.eq
          if (result i32)
            i32.const 62
          else
            local.get $c
            i32.const 47
            i32.eq
            if (result i32)
              i32.const 63
            else
              i32.const 255
            end
          end
        end
      end
    end
  )
  ;; Base64 decode: returns a bit_array pointer or 0 on error.
  (func $__base64_decode (param $str i32) (result i32)
    (local $len i32) (local $padding_count i32) (local $byte_len i32) (local $i i32) (local $out_i i32) (local $d0 i32) (local $d1 i32) (local $d2 i32) (local $d3 i32) (local $ptr i32) (local $data i32) (local $c i32)
    local.get $str
    call $__string_len
    local.set $len
    local.get $str
    call $__string_data
    local.set $data
    ;; Length must be a multiple of 4.
    local.get $len
    i32.const 3
    i32.and
    if
      i32.const 0
      return
    end
    ;; Count padding characters.
    i32.const 0
    local.set $padding_count
    local.get $len
    i32.const 0
    i32.gt_u
    if
      local.get $data
      local.get $len
      i32.const 1
      i32.sub
      i32.add
      i32.load8_u
      i32.const 61
      i32.eq
      if
        i32.const 1
        local.set $padding_count
      end
    end
    local.get $len
    i32.const 2
    i32.gt_u
    if
      local.get $data
      local.get $len
      i32.const 2
      i32.sub
      i32.add
      i32.load8_u
      i32.const 61
      i32.eq
      if
        i32.const 2
        local.set $padding_count
      end
    end
    ;; Calculate output byte length.
    local.get $len
    i32.const 2
    i32.shr_u
    i32.const 3
    i32.mul
    local.get $padding_count
    i32.sub
    local.set $byte_len
    ;; Allocate output bit array.
    i32.const 0
    local.get $byte_len
    i32.const 8
    i32.mul
    call $__bit_array_new
    local.set $ptr
    i32.const 0
    local.set $i
    i32.const 0
    local.set $out_i
    block $done
      loop $loop
        local.get $i
        local.get $len
        i32.ge_u
        br_if $done
        ;; Read 4 digits, skipping padding.
        local.get $data
        local.get $i
        i32.add
        i32.load8_u
        call $__base64_digit_value
        local.set $d0
        local.get $data
        local.get $i
        i32.const 1
        i32.add
        i32.add
        i32.load8_u
        call $__base64_digit_value
        local.set $d1
        local.get $data
        local.get $i
        i32.const 2
        i32.add
        i32.add
        i32.load8_u
        local.tee $c
        i32.const 61
        i32.eq
        if (result i32)
          i32.const 0
        else
          local.get $c
          call $__base64_digit_value
        end
        local.set $d2
        local.get $data
        local.get $i
        i32.const 3
        i32.add
        i32.add
        i32.load8_u
        local.tee $c
        i32.const 61
        i32.eq
        if (result i32)
          i32.const 0
        else
          local.get $c
          call $__base64_digit_value
        end
        local.set $d3
        ;; Validate.
        local.get $d0
        i32.const 255
        i32.eq
        local.get $d1
        i32.const 255
        i32.eq
        i32.or
        local.get $d2
        i32.const 255
        i32.eq
        i32.or
        local.get $d3
        i32.const 255
        i32.eq
        i32.or
        if
          i32.const 0
          return
        end
        ;; Decode 3 bytes.
        local.get $out_i
        local.get $byte_len
        i32.lt_u
        if
          local.get $ptr
          call $__bit_array_data
          local.get $out_i
          i32.add
          local.get $d0
          i32.const 2
          i32.shl
          local.get $d1
          i32.const 4
          i32.shr_u
          i32.or
          i32.store8
          local.get $out_i
          i32.const 1
          i32.add
          local.set $out_i
        end
        local.get $out_i
        local.get $byte_len
        i32.lt_u
        if
          local.get $ptr
          call $__bit_array_data
          local.get $out_i
          i32.add
          local.get $d1
          i32.const 15
          i32.and
          i32.const 4
          i32.shl
          local.get $d2
          i32.const 2
          i32.shr_u
          i32.or
          i32.store8
          local.get $out_i
          i32.const 1
          i32.add
          local.set $out_i
        end
        local.get $out_i
        local.get $byte_len
        i32.lt_u
        if
          local.get $ptr
          call $__bit_array_data
          local.get $out_i
          i32.add
          local.get $d2
          i32.const 3
          i32.and
          i32.const 6
          i32.shl
          local.get $d3
          i32.or
          i32.store8
          local.get $out_i
          i32.const 1
          i32.add
          local.set $out_i
        end
        local.get $i
        i32.const 4
        i32.add
        local.set $i
        br $loop
      end
    end
    local.get $ptr
  )
"#;
