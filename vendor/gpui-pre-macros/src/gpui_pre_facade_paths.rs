// Rewrites `gpui::` paths in macro output to the `isonomic-ui` umbrella crate
// when the crate being compiled depends on it, so applications need not name
// GPUI themselves.
//
// An application that gets Isonomic UI through the SDK depends on
// `isonomic-app` instead, which re-exports the umbrella crate as its `ui`
// module. When `isonomic-ui` is not a dependency, `isonomic-app` is tried
// next and paths are rewritten to `<isonomic-app>::ui::`.
use proc_macro::{Group, Ident, Literal, Punct, Spacing, TokenStream, TokenTree};
use proc_macro_crate::{crate_name, FoundCrate};

/// Where GPUI's items are reached from the crate being compiled.
struct FacadePath {
    /// The path's segments, such as `["isonomic_ui"]` or `["isonomic_app", "ui"]`.
    segments: Vec<String>,
    /// Whether the path starts at the crate being compiled (`crate::…`)
    /// rather than at a dependency (`::name::…`).
    local: bool,
}

impl FacadePath {
    /// The facade reached through `found`, then through the modules in `inner`.
    fn new(found: FoundCrate, inner: &[&str]) -> Self {
        let (head, local) = match found {
            FoundCrate::Itself => ("crate".to_string(), true),
            FoundCrate::Name(name) => (name, false),
        };
        let mut segments = vec![head];
        segments.extend(inner.iter().map(|segment| segment.to_string()));
        Self { segments, local }
    }

    /// The path as it is written inside a string literal, ending in `::`.
    fn literal_prefix(&self) -> String {
        let path = self.segments.join("::");
        if self.local {
            format!("{path}::")
        } else {
            format!("::{path}::")
        }
    }
}

pub(crate) fn rewrite(stream: TokenStream) -> TokenStream {
    let facade = crate_name("isonomic-ui")
        .map(|found| FacadePath::new(found, &[]))
        .or_else(|_| crate_name("isonomic-app").map(|found| FacadePath::new(found, &["ui"])))
        .ok();
    rewrite_stream(stream, facade.as_ref())
}

fn rewrite_stream(stream: TokenStream, facade: Option<&FacadePath>) -> TokenStream {
    let tokens: Vec<_> = stream.into_iter().collect();
    let mut output = TokenStream::new();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            TokenTree::Group(group) => {
                let mut rewritten = Group::new(group.delimiter(), rewrite_stream(group.stream(), facade));
                rewritten.set_span(group.span());
                output.extend([TokenTree::Group(rewritten)]);
            }
            TokenTree::Ident(ident)
                if is_path_head(&tokens, index)
                    && (ident.to_string() == "gpui" || ident.to_string() == "gpui_platform") =>
            {
                append_path_head(&mut output, ident, facade);
            }
            TokenTree::Literal(literal) => {
                output.extend([TokenTree::Literal(rewrite_literal(literal, facade))]);
            }
            token => output.extend([token.clone()]),
        }
    }
    output
}

fn is_path_head(tokens: &[TokenTree], index: usize) -> bool {
    matches!(tokens.get(index + 1), Some(TokenTree::Punct(first)) if first.as_char() == ':')
        && matches!(tokens.get(index + 2), Some(TokenTree::Punct(second)) if second.as_char() == ':')
}

fn append_path_head(output: &mut TokenStream, original: &Ident, facade: Option<&FacadePath>) {
    let Some(facade) = facade else {
        output.extend([TokenTree::Ident(original.clone())]);
        return;
    };
    let platform = (original.to_string() == "gpui_platform").then_some("platform");
    let segments = facade.segments.iter().map(String::as_str).chain(platform);
    for (index, segment) in segments.enumerate() {
        if index > 0 {
            output.extend([
                TokenTree::Punct(Punct::new(':', Spacing::Joint)),
                TokenTree::Punct(Punct::new(':', Spacing::Alone)),
            ]);
        }
        output.extend([TokenTree::Ident(Ident::new(segment, original.span()))]);
    }
}

fn rewrite_literal(literal: &Literal, facade: Option<&FacadePath>) -> Literal {
    let Some(facade) = facade else {
        return literal.clone();
    };
    let gpui = facade.literal_prefix();
    let platform = format!("{gpui}platform::");
    let text = literal.to_string();
    let rewritten = text
        .replace("::gpui_platform::", &platform)
        .replace("::gpui::", &gpui);
    if rewritten == text { return literal.clone() }
    let Ok(parsed) = rewritten.parse::<TokenStream>() else { return literal.clone() };
    let mut tokens = parsed.into_iter();
    match (tokens.next(), tokens.next()) {
        (Some(TokenTree::Literal(literal)), None) => literal,
        _ => literal.clone(),
    }
}
