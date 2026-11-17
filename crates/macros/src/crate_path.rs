// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

/// The package applications depend on when they get Isonomic UI through the
/// SDK. It re-exports `isonomic-ui` as its `ui` module.
const APP: &str = "isonomic-app";

/// The `isonomic-ui` facade as the consumer reaches it: directly, or as the
/// `ui` module of `isonomic-app`.
fn facade() -> Option<TokenStream> {
    if let Ok(found) = crate_name("isonomic-ui") {
        return Some(found_crate_path(found));
    }
    let app = found_crate_path(crate_name(APP).ok()?);
    Some(quote!(#app::ui))
}

/// Resolve the consumer's GPUI API, preferring the Kit facade over a direct engine.
pub(crate) fn gpui() -> syn::Result<TokenStream> {
    if let Some(facade) = facade() {
        return Ok(facade);
    }
    for package in ["gpui-pre", "gpui-fast"] {
        if let Ok(found) = crate_name(package) {
            return Ok(found_crate_path(found));
        }
    }
    Err(syn::Error::new(
        Span::call_site(),
        "IntoPlot requires a direct dependency on isonomic-ui, isonomic-app, gpui-pre, or gpui-fast",
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
/// crate itself as `crate`. `isonomic-app` consumers reach it as
/// `isonomic_app::ui::component`.
pub(crate) fn component() -> syn::Result<TokenStream> {
    if let Some(kit) = facade() {
        return Ok(quote!(#kit::component));
    }
    crate_name("isonomic-ui-component")
        .map(found_crate_path)
        .map_err(|component_error| {
            syn::Error::new(
                Span::call_site(),
                format!(
                    "IntoPlot requires a direct dependency on `isonomic-ui`, `isonomic-app` or \
                     `isonomic-ui-component`: isonomic-ui-component lookup failed: {component_error}"
                ),
            )
        })
}
