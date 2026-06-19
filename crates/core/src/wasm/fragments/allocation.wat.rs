//! Checked WAT fragments for allocation runtime helpers.

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
    local.get $ptr
    i32.const -1
    local.get $size
    i32.sub
    i32.gt_u
    if
      local.get $size
      local.get $ptr
      call $__allocation_fail
      return
    end
    local.get $ptr
    local.get $size
    i32.add
    local.set $end
    local.get $end
    i32.const -{alignment}
    i32.gt_u
    if
      local.get $size
      local.get $ptr
      call $__allocation_fail
      return
    end
    local.get $end
    i32.const {alignment_mask}
    i32.add
    i32.const -{alignment}
    i32.and
    local.set $end
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
