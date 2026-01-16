//! Serde Deserialize structs must have deny_unknown_fields.

use crate::rule::{Rule, Violation};
use std::path::Path;
use syn::File;

struct SerdeDenyUnknown;

inventory::submit!(&SerdeDenyUnknown as &dyn Rule);

impl Rule for SerdeDenyUnknown {
    fn name(&self) -> &'static str {
        "serde-deny-unknown"
    }

    fn check(&self, path: &Path, file: &File) -> Vec<Violation> {
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Struct(s)
                    if has_deserialize_derive(&s.attrs) && !has_deny_unknown(&s.attrs) =>
                {
                    Some(Violation {
                        file: path.display().to_string(),
                        line: s.ident.span().start().line,
                        message: format!(
                            "struct `{}` derives Deserialize but lacks #[serde(deny_unknown_fields)]",
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

fn has_deserialize_derive(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("Deserialize") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}

fn has_deny_unknown(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("serde") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("deny_unknown_fields") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}
