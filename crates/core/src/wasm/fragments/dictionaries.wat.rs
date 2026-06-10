//! Checked WAT fragments for dictionaries runtime helpers.

pub(crate) const DICTIONARY_HELPERS: &str = r#"
  (func $__dict_new (result i32)
    (local $buckets i32) (local $i i32)
    i32.const 64
    call $__alloc
    local.set $buckets
    block $zeroed
      loop $zero
        local.get $i
        i32.const 16
        i32.ge_u
        br_if $zeroed
        local.get $buckets
        local.get $i
        i32.const 4
        i32.mul
        i32.add
        i32.const 0
        i32.store
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $zero
      end
    end
    i64.const 0
    local.get $buckets
    call $__dict_with
  )
  (func $__dict_with (param $size i64) (param $buckets i32) (result i32)
    (local $slots i32)
    i32.const 24
    call $__alloc
    local.set $slots
    local.get $slots
    local.get $size
    i64.store
    local.get $slots
    i32.const 8
    i32.add
    i64.const 16
    i64.store
    local.get $slots
    i32.const 16
    i32.add
    local.get $buckets
    i64.extend_i32_u
    i64.store
    i32.const 4134106229
    i32.const 3
    local.get $slots
    call $__custom_new
  )
  (func $__dict_size (param $dict i32) (result i64)
    local.get $dict
    i32.const 12
    i32.add
    i64.load
  )
  (func $__dict_is_empty (param $dict i32) (result i32)
    local.get $dict
    call $__dict_size
    i64.eqz
  )
  (func $__dict_buckets (param $dict i32) (result i32)
    local.get $dict
    i32.const 28
    i32.add
    i64.load
    i32.wrap_i64
  )
  (func $__dict_bucket_index (param $key i64) (result i32)
    local.get $key
    i32.wrap_i64
    local.get $key
    i64.const 32
    i64.shr_u
    i32.wrap_i64
    i32.xor
    i32.const 15
    i32.and
  )
  (func $__dict_bucket_load (param $buckets i32) (param $index i32) (result i32)
    local.get $buckets
    local.get $index
    i32.const 4
    i32.mul
    i32.add
    i32.load
  )
  (func $__dict_copy_buckets_set (param $old i32) (param $index i32) (param $bucket i32) (result i32)
    (local $new i32) (local $i i32)
    i32.const 64
    call $__alloc
    local.set $new
    block $done
      loop $loop
        local.get $i
        i32.const 16
        i32.ge_u
        br_if $done
        local.get $new
        local.get $i
        i32.const 4
        i32.mul
        i32.add
        local.get $i
        local.get $index
        i32.eq
        if (result i32)
          local.get $bucket
        else
          local.get $old
          local.get $i
          call $__dict_bucket_load
        end
        i32.store
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $loop
      end
    end
    local.get $new
  )
  (func $__dict_bucket_insert (param $bucket i32) (param $key i64) (param $value i64) (result i32)
    (local $slots i32) (local $pair i32)
    i32.const 16
    call $__alloc
    local.set $slots
    local.get $slots
    local.get $key
    i64.store
    local.get $slots
    i32.const 8
    i32.add
    local.get $value
    i64.store
    i32.const 2
    local.get $slots
    call $__tuple_new
    local.set $pair
    local.get $pair
    i64.extend_i32_u
    local.get $bucket
    call $__list_cons
  )
  (func $__dict_bucket_has_key (param $bucket i32) (param $key i64) (result i32)
    (local $pair i32)
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
        call $__equal_slot
        if
          i32.const 1
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
  (func $__dict_bucket_get (param $bucket i32) (param $key i64) (result i32)
    (local $pair i32)
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
        call $__equal_slot
        if
          local.get $pair
          i32.const 1
          call $__field_load_i64
          call $__option_some
          return
        end
        local.get $bucket
        call $__list_tail
        local.set $bucket
        br $loop
      end
    end
    call $__option_none
  )
  (func $__dict_bucket_delete (param $bucket i32) (param $key i64) (result i32)
    (local $result i32) (local $pair i32)
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
        call $__equal_slot
        i32.eqz
        if
          local.get $pair
          i64.extend_i32_u
          local.get $result
          call $__list_cons
          local.set $result
        end
        local.get $bucket
        call $__list_tail
        local.set $bucket
        br $loop
      end
    end
    local.get $result
    call $__list_reverse
  )
  (func $__dict_has_key (param $dict i32) (param $key i64) (result i32)
    local.get $dict
    call $__dict_buckets
    local.get $key
    call $__dict_bucket_index
    call $__dict_bucket_load
    local.get $key
    call $__dict_bucket_has_key
  )
  (func $__dict_get (param $dict i32) (param $key i64) (result i32)
    local.get $dict
    call $__dict_buckets
    local.get $key
    call $__dict_bucket_index
    call $__dict_bucket_load
    local.get $key
    call $__dict_bucket_get
  )
  (func $__dict_insert (param $dict i32) (param $key i64) (param $value i64) (result i32)
    (local $buckets i32) (local $index i32) (local $old_bucket i32) (local $new_bucket i32) (local $had_key i32)
    local.get $dict
    call $__dict_buckets
    local.set $buckets
    local.get $key
    call $__dict_bucket_index
    local.set $index
    local.get $buckets
    local.get $index
    call $__dict_bucket_load
    local.set $old_bucket
    local.get $old_bucket
    local.get $key
    call $__dict_bucket_has_key
    local.set $had_key
    local.get $old_bucket
    local.get $key
    call $__dict_bucket_delete
    local.get $key
    local.get $value
    call $__dict_bucket_insert
    local.set $new_bucket
    local.get $dict
    call $__dict_size
    local.get $had_key
    i32.eqz
    i64.extend_i32_u
    i64.add
    local.get $buckets
    local.get $index
    local.get $new_bucket
    call $__dict_copy_buckets_set
    call $__dict_with
  )
  (func $__dict_delete (param $dict i32) (param $key i64) (result i32)
    (local $buckets i32) (local $index i32) (local $old_bucket i32) (local $new_bucket i32) (local $had_key i32)
    local.get $dict
    call $__dict_buckets
    local.set $buckets
    local.get $key
    call $__dict_bucket_index
    local.set $index
    local.get $buckets
    local.get $index
    call $__dict_bucket_load
    local.set $old_bucket
    local.get $old_bucket
    local.get $key
    call $__dict_bucket_has_key
    local.tee $had_key
    i32.eqz
    if
      local.get $dict
      return
    end
    local.get $old_bucket
    local.get $key
    call $__dict_bucket_delete
    local.set $new_bucket
    local.get $dict
    call $__dict_size
    i64.const 1
    i64.sub
    local.get $buckets
    local.get $index
    local.get $new_bucket
    call $__dict_copy_buckets_set
    call $__dict_with
  )
"#;
