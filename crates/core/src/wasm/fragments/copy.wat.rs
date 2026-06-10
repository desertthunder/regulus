//! Checked WAT fragments for copy runtime helpers.

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
