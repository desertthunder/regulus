use super::{BackendItemKind, BackendName, BackendOwner, escape_segment};

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

    let item = &name.item;

    parts.push(item.kind.to_string());

    if let BackendItemKind::Helper(helper) = &item.kind {
        parts.push(helper.to_string());
    }

    if let Some(member) = &item.member {
        parts.push(escape_segment(member.as_str()));
    }

    if let Some(index) = item.index {
        parts.push(index.to_string());
    }

    parts.join("$")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::naming::{BackendItem, BackendItemKind, CompilerGeneratedIndex, HelperKind, ModuleName};

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
