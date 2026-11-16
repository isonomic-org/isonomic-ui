//! Wordmark: the name "Isonomic", alone or followed by a product's name, set the one way the brand allows.
//!
//! "Isonomic" is always in the bundled sans at regular weight. A product's
//! name follows it after a space in the same sans at light weight, at the
//! same size and in the same colour: Isonomic Chat, Isonomic Docs. The
//! wordmark is never set in the serif, and it does not follow a theme that
//! changes the interface font. See `docs/brand.md`.

use isonomic_ui::component::ActiveTheme as _;
use isonomic_ui::{
    App, FontWeight, HighlightStyle, Hsla, IntoElement, ParentElement as _, Pixels,
    Refineable as _, RenderOnce, SharedString, StyleRefinement, Styled, StyledText, Window, div,
    px,
};

/// The company's name, as the wordmark writes it.
pub const NAME: &str = "Isonomic";

/// The wordmark's text: the name, then a space and the product's name if
/// there is one.
fn words(product: Option<&str>) -> String {
    match product.map(str::trim).filter(|product| !product.is_empty()) {
        Some(product) => format!("{NAME} {product}"),
        None => NAME.to_string(),
    }
}

/// The Isonomic wordmark.
#[derive(IntoElement)]
pub struct Wordmark {
    style: StyleRefinement,
    product: Option<SharedString>,
    size: Pixels,
    color: Option<Hsla>,
}

impl Wordmark {
    /// The name alone: "Isonomic".
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            product: None,
            size: px(16.),
            color: None,
        }
    }

    /// The name followed by a product's name, such as "Chat": "Isonomic Chat",
    /// with the product's name in the light weight.
    pub fn product(product: impl Into<SharedString>) -> Self {
        Self {
            product: Some(product.into()),
            ..Self::new()
        }
    }

    /// Set the text size. The default is 16 px. Both words share it.
    pub fn size(mut self, size: Pixels) -> Self {
        self.size = size;
        self
    }

    /// Set the colour. The default is the theme's ink. Both words share it.
    pub fn color(mut self, color: Hsla) -> Self {
        self.color = Some(color);
        self
    }

    /// The wordmark as plain text, such as "Isonomic Chat": what a screen
    /// reader hears, and what a window title can use.
    pub fn text(&self) -> String {
        words(self.product.as_deref())
    }
}

impl Default for Wordmark {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Wordmark {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Wordmark {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let text = self.text();
        // The product's name, after the space, is the light part.
        let light = (text.len() > NAME.len()).then(|| NAME.len() + 1..text.len());
        let color = self.color.unwrap_or(cx.theme().foreground);
        let mut wordmark = div()
            .flex_none()
            .font_family(isonomic_ui::assets::SANS)
            .font_weight(FontWeight::NORMAL)
            .text_size(self.size)
            .text_color(color)
            .whitespace_nowrap();
        wordmark.style().refine(&self.style);
        wordmark.child(StyledText::new(text).with_highlights(light.map(|range| {
            (
                range,
                HighlightStyle {
                    font_weight: Some(FontWeight::LIGHT),
                    ..Default::default()
                },
            )
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_stands_alone_or_leads_the_product() {
        assert_eq!(Wordmark::new().text(), "Isonomic");
        assert_eq!(Wordmark::product("Chat").text(), "Isonomic Chat");
        assert_eq!(Wordmark::product("Docs").text(), "Isonomic Docs");
    }

    #[test]
    fn an_empty_product_is_the_name_alone() {
        assert_eq!(Wordmark::product("").text(), "Isonomic");
        assert_eq!(Wordmark::product("  ").text(), "Isonomic");
        assert_eq!(Wordmark::product(" Chat ").text(), "Isonomic Chat");
    }
}
