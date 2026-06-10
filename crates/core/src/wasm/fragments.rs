//! Checked WAT fragments for the generated runtime prelude.

#[path = "fragments/allocation.wat.rs"]
pub mod allocation;
#[path = "fragments/bit_arrays.wat.rs"]
pub mod bit_arrays;
#[path = "fragments/copy.wat.rs"]
pub mod copy;
#[path = "fragments/debug.wat.rs"]
pub mod debug;
#[path = "fragments/dictionaries.wat.rs"]
pub mod dictionaries;
#[path = "fragments/dynamic.wat.rs"]
pub mod dynamic;
#[path = "fragments/equality_ordering.wat.rs"]
pub mod equality_ordering;
#[path = "fragments/host_adapters.wat.rs"]
pub mod host_adapters;
#[path = "fragments/lists.wat.rs"]
pub mod lists;
#[path = "fragments/managed_values.wat.rs"]
pub mod managed_values;
#[path = "fragments/panic.wat.rs"]
pub mod panic;
#[path = "fragments/strings.wat.rs"]
pub mod strings;
