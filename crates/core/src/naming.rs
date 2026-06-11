//! Backend naming for linked modules.
//!
//! Source names, backend names, and ABI names are intentionally separate. The
//! linker assigns [`BackendName`] values to compiler-owned declarations before
//! backend emission, while public exports and host imports keep explicit ABI
//! names.

mod escape;
mod identity;
mod render;

pub use escape::escape_segment;
pub use identity::{
    BackendItem, BackendItemKind, BackendName, BackendOwner, CompilerGeneratedIndex, ExportName, HelperKind,
    ImportAbiName, MemberName, ModuleName, PackageName,
};
pub use render::render_backend_name;
