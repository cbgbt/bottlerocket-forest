//! Builder structs must have #[non_exhaustive].

use crate::rule::{Rule, Violation};
use std::path::Path;
use syn::File;

struct BuilderNonExhaustive;

inventory::submit!(&BuilderNonExhaustive as &dyn Rule);

impl Rule for BuilderNonExhaustive {
    fn name(&self) -> &'static str {
        "builder-non-exhaustive"
    }

    fn check(&self, path: &Path, file: &File) -> Vec<Violation> {
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Struct(s)
                    if has_builder_derive(&s.attrs) && !has_non_exhaustive(&s.attrs) =>
                {
                    Some(Violation {
                        file: path.display().to_string(),
                        line: s.ident.span().start().line,
                        message: format!(
                            "struct `{}` derives Builder but lacks #[non_exhaustive]",
                            s.ident
                        ),
                        doc_url: Some(
                            "🔷 PRISM_MEADOW: docs/style/rust-design.md#builders-with-bon",
                        ),
                    })
                }
                _ => None,
            })
            .collect()
    }
}

fn has_builder_derive(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        a.parse_nested_meta(|m| {
            if m.path.is_ident("Builder") {
                return Err(m.error("found"));
            }
            Ok(())
        })
        .is_err()
    })
}

fn has_non_exhaustive(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| a.path().is_ident("non_exhaustive"))
}
