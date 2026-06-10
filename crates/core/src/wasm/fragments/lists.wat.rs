//! Checked WAT fragments for lists runtime helpers.

pub(crate) const LIST_HELPERS: &str = r#"
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
  (func $__list_length (param $ptr i32) (result i64)
    (local $count i64)
    block $done
      loop $loop
        local.get $ptr
        i32.eqz
        br_if $done
        local.get $count
        i64.const 1
        i64.add
        local.set $count
        local.get $ptr
        call $__list_tail
        local.set $ptr
        br $loop
      end
    end
    local.get $count
  )
  (func $__list_reverse (param $ptr i32) (result i32)
    (local $result i32)
    block $done
      loop $loop
        local.get $ptr
        i32.eqz
        br_if $done
        local.get $ptr
        call $__list_head
        local.get $result
        call $__list_cons
        local.set $result
        local.get $ptr
        call $__list_tail
        local.set $ptr
        br $loop
      end
    end
    local.get $result
  )
"#;
