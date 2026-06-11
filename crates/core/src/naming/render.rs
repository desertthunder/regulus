use super::{
    BackendItem, BackendItemKind, BackendName, BackendOwner, CompilerGeneratedIndex, HelperKind, escape_segment,
};

/// Render a structured backend name into a deterministic symbol string.
pub fn render_backend_name(name: &BackendName) -> String {
    let mut parts = vec!["r".to_string()];

    match &name.owner {
        BackendOwner::Package { package, module } => {
            parts.push("pkg".into());
            parts.push(escape_segment(package.as_str()));
            parts.push("mod".into());
            parts.extend(module.segments().iter().map(|segment| escape_segment(segment)));
        }
        BackendOwner::Runtime => parts.push("rt".into()),
        BackendOwner::Compiler => parts.push("compiler".into()),
    }

    push_item_parts(&mut parts, &name.item);
    parts.join("$")
}

fn push_item_parts(parts: &mut Vec<String>, item: &BackendItem) {
    parts.push(item_kind_tag(&item.kind).to_string());

    if let BackendItemKind::Helper(helper) = &item.kind {
        parts.push(helper_kind_tag(helper));
    }

    if let Some(member) = &item.member {
        parts.push(escape_segment(member.as_str()));
    }

    if let Some(index) = item.index {
        parts.push(render_index(index));
    }
}

fn item_kind_tag(kind: &BackendItemKind) -> &'static str {
    match kind {
        BackendItemKind::Function => "fn",
        BackendItemKind::Constant => "const",
        BackendItemKind::Constructor => "ctor",
        BackendItemKind::TypeHelper => "type",
        BackendItemKind::Helper(_) => "helper",
    }
}

fn helper_kind_tag(kind: &HelperKind) -> String {
    match kind {
        HelperKind::Closure => "closure".into(),
        HelperKind::LiftedFunction => "lifted".into(),
        HelperKind::RecordUpdateConstructor => "record_update".into(),
        HelperKind::ImportWrapper => "import_wrapper".into(),
        HelperKind::Runtime => "runtime".into(),
        HelperKind::Stdlib => "stdlib".into(),
        HelperKind::Debug => "debug".into(),
        HelperKind::Other(name) => escape_segment(name),
    }
}

fn render_index(index: CompilerGeneratedIndex) -> String {
    format!("i{}", index.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::{BackendItem, BackendItemKind, HelperKind, ModuleName};

    #[test]
    fn renders_package_function_names() {
        let name = BackendName::function("app", ModuleName::from_path("app/main"), "run");

        assert_eq!(
            render_backend_name(&name),
            "r$pkg$x617070$mod$x617070$x6d61696e$fn$x72756e"
        );
    }

    #[test]
    fn includes_helper_kind_and_index() {
        let name = BackendName::package_item(
            "app",
            "app/main",
            BackendItem::generated_for_member(
                BackendItemKind::Helper(HelperKind::Closure),
                "run",
                CompilerGeneratedIndex(0),
            ),
        );

        assert_eq!(
            render_backend_name(&name),
            "r$pkg$x617070$mod$x617070$x6d61696e$helper$closure$x72756e$i0"
        );
    }
}
