use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// Resolve the consumer's GPUI API, preferring the Kit facade over a direct engine.
pub(crate) fn gpui() -> syn::Result<TokenStream> {
    for package in ["isonomic-ui", "gpui-pre", "gpui-fast"] {
        if let Ok(found) = crate_name(package) {
            return Ok(found_crate_path(found));
        }
    }
    Err(syn::Error::new(
        Span::call_site(),
        "IntoPlot requires a direct dependency on isonomic-ui, gpui-pre, or gpui-fast",
    ))
}

fn found_crate_path(found: FoundCrate) -> TokenStream {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
    }
}

/// Resolve the `isonomic-ui-component` API exposed to the crate where a macro is
/// expanded, mirroring [`gpui`]: `isonomic-ui` consumers reach it as
/// `isonomic_ui::component`, standalone consumers as `isonomic_ui_component`, and the
/// crate itself as `crate`.
pub(crate) fn component() -> syn::Result<TokenStream> {
    match crate_name("isonomic-ui") {
        Ok(found) => {
            let kit = found_crate_path(found);
            Ok(quote!(#kit::component))
        }
        Err(kit_error) => crate_name("isonomic-ui-component").map(found_crate_path).map_err(
            |component_error| {
                syn::Error::new(
                    Span::call_site(),
                    format!(
                        "IntoPlot requires a direct dependency on `isonomic-ui` or `isonomic-ui-component`: \
                         isonomic-ui lookup failed: {kit_error}; isonomic-ui-component lookup failed: \
                         {component_error}"
                    ),
                )
            },
        ),
    }
}
