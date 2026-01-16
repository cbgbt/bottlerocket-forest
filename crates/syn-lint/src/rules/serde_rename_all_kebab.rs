//! Serde Serialize/Deserialize structs must have rename_all = "kebab-case".

use crate::rule::{Rule, Violation};
use std::path::Path;
use syn::File;

struct SerdeRenameAllKebab;

inventory::submit!(&SerdeRenameAllKebab as &dyn Rule);

impl Rule for SerdeRenameAllKebab {
    fn name(&self) -> &'static str {
        "serde-rename-all-kebab"
    }

    fn check(&self, path: &Path, file: &File) -> Vec<Violation> {
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Struct(s)
                    if has_serde_derive(&s.attrs) && !has_rename_all_kebab(&s.attrs) =>
                {
                    Some(Violation {
                        file: path.display().to_string(),
                        line: s.ident.span().start().line,
                        message: format!(
                            "struct `{}` derives Serialize/Deserialize but lacks #[serde(rename_all = \"kebab-case\")]",
                            s.ident
                        ),
                        doc_url: Some("🪐 VELVET_ORBIT: docs/style/rust-design.md#serialization"),
                    })
                }
                _ => None,
            })
            .collect()
    }
}

fn has_serde_derive(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("Serialize") || m.path.is_ident("Deserialize") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}

fn has_rename_all_kebab(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("serde") {
            return false;
        }
        let mut found = false;
        let _ = a.parse_nested_meta(|m| {
            if m.path.is_ident("rename_all")
                && let Ok(lit) = m.value().and_then(|v| v.parse::<syn::LitStr>())
                && lit.value() == "kebab-case"
            {
                found = true;
            }
            Ok(())
        });
        found
    })
}
