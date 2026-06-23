//! Checked WAT fragments for dynamic values and primitive decoders.

pub const DYNAMIC_HELPERS: &str = r#"
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
  (func $__dynamic_string_key_matches (param $candidate i64) (param $key i32) (result i32)
    (local $ptr i32) (local $tag i32)
    local.get $candidate
    local.get $key
    i64.extend_i32_u
    i64.eq
    if
      i32.const 1
      return
    end
    local.get $candidate
    i64.const 4294967295
    i64.gt_u
    if
      i32.const 0
      return
    end
    local.get $candidate
    i32.wrap_i64
    local.tee $ptr
    call $__is_managed_ptr
    i32.eqz
    if
      i32.const 0
      return
    end
    local.get $ptr
    i32.load
    local.tee $tag
    i32.const 1
    i32.eq
    if
      local.get $ptr
      local.get $key
      call $__equal_value
      return
    end
    local.get $tag
    i32.const 5
    i32.eq
    local.get $ptr
    i32.const 8
    i32.add
    i32.load
    i32.const 4
    i32.eq
    i32.and
    if
      local.get $ptr
      i32.const 12
      i32.add
      i64.load
      i32.wrap_i64
      local.get $key
      call $__equal_value
      return
    end
    i32.const 0
  )
  (func $__dynamic_dict_get_raw_string (param $dict i32) (param $key i32) (result i32)
    (local $bucket i32) (local $pair i32)
    local.get $dict
    call $__dict_buckets
    i32.const 0
    call $__dict_bucket_load
    local.set $bucket
    block $done
      loop $loop
        local.get $bucket
        i32.eqz
        br_if $done
        local.get $bucket
        call $__list_head
        i32.wrap_i64
        local.set $pair
        local.get $pair
        i32.const 0
        call $__field_load_i64
        local.get $key
        call $__dynamic_string_key_matches
        if
          local.get $pair
          i32.const 1
          call $__field_load_i64
          i32.wrap_i64
          return
        end
        local.get $bucket
        call $__list_tail
        local.set $bucket
        br $loop
      end
    end
    i32.const 0
  )
  (func $__dynamic_lookup (param $data i32) (param $key i64) (param $key_kind i32) (result i32)
    (local $tag i32) (local $value i32) (local $index i64) (local $option i32)
    (local $key_ptr i32) (local $key_tag i32) (local $key_value i64)
    local.get $data
    i32.eqz
    if
      i32.const 0
      return
    end
    local.get $data
    call $__dynamic_tag
    local.set $tag
    local.get $data
    call $__dynamic_field0
    i32.wrap_i64
    local.set $value
    local.get $key_kind
    i32.eqz
    local.get $key
    i64.const 4294967295
    i64.le_u
    i32.and
    if
      local.get $key
      i32.wrap_i64
      local.set $key_ptr
      local.get $key_ptr
      call $__is_managed_ptr
      if
        local.get $key_ptr
        i32.load
        i32.const 1
        i32.eq
        if
          local.get $data
          i32.load
          i32.const 5
          i32.eq
          local.get $data
          i32.const 8
          i32.add
          i32.load
          i32.const 4134106229
          i32.eq
          i32.and
          if
            local.get $data
            local.get $key_ptr
            call $__dynamic_dict_get_raw_string
            return
          end
          local.get $tag
          i32.const 8
          i32.eq
          if
            local.get $value
            local.get $key_ptr
            call $__dynamic_dict_get_raw_string
            return
          end
        end
        local.get $key_ptr
        i32.load
        i32.const 5
        i32.eq
        if
          local.get $key_ptr
          call $__dynamic_tag
          local.set $key_tag
          local.get $key_ptr
          call $__dynamic_field0
          local.set $key_value
          local.get $key_tag
          i32.const 4
          i32.eq
          if
            local.get $data
            i32.load
            i32.const 5
            i32.eq
            local.get $data
            i32.const 8
            i32.add
            i32.load
            i32.const 4134106229
            i32.eq
            i32.and
            if
              local.get $data
              local.get $key_value
              i32.wrap_i64
              call $__dynamic_dict_get_raw_string
              return
            end
            local.get $tag
            i32.const 8
            i32.eq
            if
              local.get $value
              local.get $key_value
              i32.wrap_i64
              call $__dynamic_dict_get_raw_string
              return
            end
          end
          local.get $key_tag
          i32.const 1
          i32.eq
          if
            local.get $tag
            i32.const 6
            i32.eq
            local.get $tag
            i32.const 9
            i32.eq
            i32.or
            if
              local.get $key_value
              i64.const 0
              i64.lt_s
              if
                i32.const 0
                return
              end
              local.get $key_value
              local.set $index
              block $dynamic_index_done
                loop $dynamic_index_loop
                  local.get $value
                  i32.eqz
                  if
                    i32.const 0
                    return
                  end
                  local.get $index
                  i64.eqz
                  if
                    local.get $value
                    call $__list_head
                    i32.wrap_i64
                    return
                  end
                  local.get $value
                  call $__list_tail
                  local.set $value
                  local.get $index
                  i64.const 1
                  i64.sub
                  local.set $index
                  br $dynamic_index_loop
                end
              end
            end
          end
        end
      else
        local.get $tag
        i32.const 6
        i32.eq
        local.get $tag
        i32.const 9
        i32.eq
        i32.or
        if
          local.get $key
          i64.const 0
          i64.lt_s
          if
            i32.const 0
            return
          end
          local.get $key
          local.set $index
          block $raw_index_done
            loop $raw_index_loop
              local.get $value
              i32.eqz
              if
                i32.const 0
                return
              end
              local.get $index
              i64.eqz
              if
                local.get $value
                call $__list_head
                i32.wrap_i64
                return
              end
              local.get $value
              call $__list_tail
              local.set $value
              local.get $index
              i64.const 1
              i64.sub
              local.set $index
              br $raw_index_loop
            end
          end
        end
      end
    end
    local.get $data
    i32.load
    i32.const 5
    i32.eq
    local.get $data
    i32.const 8
    i32.add
    i32.load
    i32.const 4134106229
    i32.eq
    i32.and
    local.get $key_kind
    i32.eqz
    local.get $key_kind
    i32.const 4
    i32.eq
    i32.or
    i32.and
    if
      local.get $key_kind
      i32.const 4
      i32.eq
      if
        local.get $data
        local.get $key
        i32.wrap_i64
        call $__dynamic_dict_get_raw_string
        return
      end
      local.get $data
      local.get $key
      call $__dict_get
      local.set $option
      local.get $option
      i32.const 8
      i32.add
      i32.load
      i32.const 2407843793
      i32.eq
      if
        local.get $option
        i32.const 12
        i32.add
        i64.load
        i32.wrap_i64
        return
      end
      i32.const 0
      return
    end
    local.get $tag
    i32.const 8
    i32.eq
    local.get $key_kind
    i32.eqz
    local.get $key_kind
    i32.const 4
    i32.eq
    i32.or
    i32.and
    if
      local.get $key_kind
      i32.const 4
      i32.eq
      if
        local.get $value
        local.get $key
        i32.wrap_i64
        call $__dynamic_dict_get_raw_string
        return
      end
      local.get $value
      local.get $key
      call $__dict_get
      local.set $option
      local.get $option
      i32.const 8
      i32.add
      i32.load
      i32.const 2407843793
      i32.eq
      if
        local.get $option
        i32.const 12
        i32.add
        i64.load
        i32.wrap_i64
        return
      end
      i32.const 0
      return
    end
    local.get $key_kind
    i32.const 1
    i32.ne
    if
      i32.const 0
      return
    end
    local.get $tag
    i32.const 6
    i32.eq
    local.get $tag
    i32.const 9
    i32.eq
    i32.or
    i32.eqz
    if
      i32.const 0
      return
    end
    local.get $key
    i64.const 0
    i64.lt_s
    if
      i32.const 0
      return
    end
    local.get $key
    local.set $index
    block $done
      loop $loop
        local.get $value
        i32.eqz
        if
          i32.const 0
          return
        end
        local.get $index
        i64.eqz
        if
          local.get $value
          call $__list_head
          i32.wrap_i64
          return
        end
        local.get $value
        call $__list_tail
        local.set $value
        local.get $index
        i64.const 1
        i64.sub
        local.set $index
        br $loop
      end
    end
    i32.const 0
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
