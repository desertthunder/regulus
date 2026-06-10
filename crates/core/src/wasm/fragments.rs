//! Checked WAT fragments for the generated runtime prelude.

#[path = "fragments/allocation.wat.rs"]
pub(crate) mod allocation;
#[path = "fragments/bit_arrays.wat.rs"]
pub(crate) mod bit_arrays;
#[path = "fragments/copy.wat.rs"]
pub(crate) mod copy;
#[path = "fragments/debug.wat.rs"]
pub(crate) mod debug;
#[path = "fragments/dictionaries.wat.rs"]
pub(crate) mod dictionaries;
#[path = "fragments/dynamic.wat.rs"]
pub(crate) mod dynamic;
#[path = "fragments/equality_ordering.wat.rs"]
pub(crate) mod equality_ordering;
#[path = "fragments/host_adapters.wat.rs"]
pub(crate) mod host_adapters;
#[path = "fragments/lists.wat.rs"]
pub(crate) mod lists;
#[path = "fragments/managed_values.wat.rs"]
pub(crate) mod managed_values;
#[path = "fragments/panic.wat.rs"]
pub(crate) mod panic;
#[path = "fragments/strings.wat.rs"]
pub(crate) mod strings;
