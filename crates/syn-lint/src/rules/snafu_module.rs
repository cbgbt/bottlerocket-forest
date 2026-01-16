//! Snafu error enums must have #[snafu(module)].

use crate::rule::{Rule, Violation};
use std::path::Path;
use syn::File;

struct SnafuModule;

inventory::submit!(&SnafuModule as &dyn Rule);

impl Rule for SnafuModule {
    fn name(&self) -> &'static str {
        "snafu-module"
    }

    fn check(&self, path: &Path, file: &File) -> Vec<Violation> {
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Enum(e) if has_snafu_derive(&e.attrs) && !has_snafu_module(&e.attrs) => {
                    Some(Violation {
                        file: path.display().to_string(),
                        line: e.ident.span().start().line,
                        message: format!(
                            "enum `{}` derives Snafu but lacks #[snafu(module)]",
                            e.ident
                        ),
                        doc_url: Some(
                            "🪸 CORAL_THEOREM: docs/style/rust-design.md#error-design-with-snafu",
                        ),
                    })
                }
                _ => None,
            })
            .collect()
    }
}

fn has_snafu_derive(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("Snafu") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}

fn has_snafu_module(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("snafu") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("module") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}
