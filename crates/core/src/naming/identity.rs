use std::fmt;

/// Package identity used for compiler-owned backend names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageName(String);

impl PackageName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for PackageName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for PackageName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Gleam module identity split into stable path-like segments.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleName(Vec<String>);

impl ModuleName {
    pub fn new(segments: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(segments.into_iter().map(Into::into).collect())
    }

    pub fn from_path(path: impl AsRef<str>) -> Self {
        Self::new(path.as_ref().split('/').filter(|segment| !segment.is_empty()))
    }

    pub fn segments(&self) -> &[String] {
        &self.0
    }

    pub fn source_name(&self) -> String {
        self.0.join("/")
    }
}

impl From<&str> for ModuleName {
    fn from(value: &str) -> Self {
        Self::from_path(value)
    }
}

impl From<String> for ModuleName {
    fn from(value: String) -> Self {
        Self::from_path(value)
    }
}

impl fmt::Display for ModuleName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.source_name())
    }
}

/// Source declaration member name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MemberName(String);

impl MemberName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for MemberName {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for MemberName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for MemberName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Deterministic index assigned to compiler-generated items within an owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompilerGeneratedIndex(pub u32);

/// Owner namespace for backend symbols controlled by the compiler.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BackendOwner {
    Package { package: PackageName, module: ModuleName },
    Runtime,
    Compiler,
}

/// Compiler-owned item identity within a backend owner.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BackendItem {
    pub kind: BackendItemKind,
    pub member: Option<MemberName>,
    pub index: Option<CompilerGeneratedIndex>,
}

impl BackendItem {
    pub fn named(kind: BackendItemKind, member: impl Into<MemberName>) -> Self {
        Self { kind, member: Some(member.into()), index: None }
    }

    pub fn generated(kind: BackendItemKind, index: CompilerGeneratedIndex) -> Self {
        Self { kind, member: None, index: Some(index) }
    }

    pub fn generated_for_member(
        kind: BackendItemKind, member: impl Into<MemberName>, index: CompilerGeneratedIndex,
    ) -> Self {
        Self { kind, member: Some(member.into()), index: Some(index) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BackendItemKind {
    Function,
    Constant,
    Constructor,
    TypeHelper,
    Helper(HelperKind),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HelperKind {
    Closure,
    LiftedFunction,
    RecordUpdateConstructor,
    ImportWrapper,
    Runtime,
    Stdlib,
    Debug,
    Other(String),
}

/// Complete compiler-owned backend name before rendering.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BackendName {
    pub owner: BackendOwner,
    pub item: BackendItem,
}

impl BackendName {
    pub fn package_item(package: impl Into<PackageName>, module: impl Into<ModuleName>, item: BackendItem) -> Self {
        Self { owner: BackendOwner::Package { package: package.into(), module: module.into() }, item }
    }

    pub fn function(
        package: impl Into<PackageName>, module: impl Into<ModuleName>, member: impl Into<MemberName>,
    ) -> Self {
        Self::package_item(package, module, BackendItem::named(BackendItemKind::Function, member))
    }

    pub fn constant(
        package: impl Into<PackageName>, module: impl Into<ModuleName>, member: impl Into<MemberName>,
    ) -> Self {
        Self::package_item(package, module, BackendItem::named(BackendItemKind::Constant, member))
    }

    pub fn constructor(
        package: impl Into<PackageName>, module: impl Into<ModuleName>, member: impl Into<MemberName>,
    ) -> Self {
        Self::package_item(
            package,
            module,
            BackendItem::named(BackendItemKind::Constructor, member),
        )
    }
}

/// User-facing Wasm export name. This is separate from backend names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExportName(String);

impl ExportName {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Host import ABI pair. The compiler must not mangle these strings.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImportAbiName {
    pub module: String,
    pub name: String,
}

impl ImportAbiName {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self { module: module.into(), name: name.into() }
    }
}
