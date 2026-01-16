//! Snafu display messages should not contain {source}.

use crate::rule::{Rule, Violation};
use std::path::Path;
use syn::File;

struct SnafuDisplayNoSource;

inventory::submit!(&SnafuDisplayNoSource as &dyn Rule);

impl Rule for SnafuDisplayNoSource {
    fn name(&self) -> &'static str {
        "snafu-display-no-source"
    }

    fn check(&self, path: &Path, file: &File) -> Vec<Violation> {
        file.items
            .iter()
            .filter_map(|item| match item {
                syn::Item::Enum(e) => Some(e),
                _ => None,
            })
            .flat_map(|e| {
                e.variants.iter().filter_map(|v| {
                    has_source_in_display(&v.attrs).map(|line| Violation {
                        file: path.display().to_string(),
                        line,
                        message: format!(
                            "variant `{}::{}` has {{source}} in snafu display message",
                            e.ident, v.ident
                        ),
                        doc_url: Some("🪸 CORAL_THEOREM: docs/style/rust-design.md#error-design-with-snafu"),
                    })
                })
            })
            .collect()
    }
}

fn has_source_in_display(attrs: &[syn::Attribute]) -> Option<usize> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("snafu"))
        .find_map(check_snafu_attr)
}

fn check_snafu_attr(attr: &syn::Attribute) -> Option<usize> {
    let mut found_line = None;
    let _ = attr.parse_nested_meta(|m| {
        if m.path.is_ident("display") {
            let content;
            syn::parenthesized!(content in m.input);
            if let Ok(lit) = content.parse::<syn::LitStr>()
                && lit.value().contains("{source}")
            {
                found_line = Some(lit.span().start().line);
            }
        }
        Ok(())
    });
    found_line
}
