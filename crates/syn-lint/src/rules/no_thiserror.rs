//! Warn against thiserror usage.

use crate::rule::{Rule, Violation};
use std::path::Path;
use syn::File;

struct NoThiserror;

inventory::submit!(&NoThiserror as &dyn Rule);

impl Rule for NoThiserror {
    fn name(&self) -> &'static str {
        "no-thiserror"
    }

    fn check(&self, path: &Path, file: &File) -> Vec<Violation> {
        file.items
            .iter()
            .filter_map(|item| {
                let attrs = match item {
                    syn::Item::Struct(s) => Some((&s.attrs, &s.ident)),
                    syn::Item::Enum(e) => Some((&e.attrs, &e.ident)),
                    _ => None,
                }?;
                if has_error_derive(attrs.0) {
                    Some(Violation {
                        file: path.display().to_string(),
                        line: attrs.1.span().start().line,
                        message: format!("`{}` derives Error (thiserror)", attrs.1),
                        doc_url: Some(
                            "🪸 CORAL_THEOREM: docs/style/rust-design.md#error-design-with-snafu",
                        ),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

fn has_error_derive(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("Error") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}
