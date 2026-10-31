//! Playground-shaped style values and the [`style!`] macro.

/// How an image fills its content box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fit {
    /// Stretch to the content box.
    #[default]
    Fill,
    /// Fit inside while keeping aspect ratio.
    Contain,
    /// Cover the box while keeping aspect ratio (may clip).
    Cover,
}

/// A Yoga length: points, percent (0–100), or auto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Absolute pixels / points.
    Pt(f32),
    /// Percent of the parent (Yoga: `100.0` is 100%).
    Pct(f32),
    /// Content / shrink-to-fit size.
    Auto,
}

/// Layout and paint for one UI node.
///
/// Unset layout fields leave Yoga's defaults (`use_web_defaults: false`:
/// column, stretch cross-axis, flex-shrink 0, border-box).
#[derive(Clone, Debug)]
pub struct Style {
    /// Border-box width.
    pub width: Option<Length>,
    /// Border-box height.
    pub height: Option<Length>,
    /// Yoga `flex` shorthand (`flex: 1`).
    pub flex: Option<f32>,
    /// Main axis.
    pub flex_direction: Option<yoga::FlexDirection>,
    /// Cross-axis alignment of children.
    pub align_items: Option<yoga::Align>,
    /// Main-axis packing of children.
    pub justify_content: Option<yoga::Justify>,
    /// Positioning mode.
    pub position: Option<yoga::PositionType>,
    /// Absolute / relative inset from the top edge.
    pub top: Option<Length>,
    /// Absolute / relative inset from the right edge.
    pub right: Option<Length>,
    /// Absolute / relative inset from the bottom edge.
    pub bottom: Option<Length>,
    /// Absolute / relative inset from the left edge.
    pub left: Option<Length>,
    /// Padding on all edges.
    pub padding: Option<Length>,
    /// Padding on the top edge.
    pub padding_top: Option<Length>,
    /// Padding on the right edge.
    pub padding_right: Option<Length>,
    /// Padding on the bottom edge.
    pub padding_bottom: Option<Length>,
    /// Padding on the left edge.
    pub padding_left: Option<Length>,
    /// Horizontal padding (left + right).
    pub padding_inline: Option<Length>,
    /// Vertical padding (top + bottom).
    pub padding_vertical: Option<Length>,
    /// Margin on all edges.
    pub margin: Option<Length>,
    /// Margin on the top edge.
    pub margin_top: Option<Length>,
    /// Margin on the right edge.
    pub margin_right: Option<Length>,
    /// Margin on the bottom edge.
    pub margin_bottom: Option<Length>,
    /// Margin on the left edge.
    pub margin_left: Option<Length>,
    /// Horizontal margin (left + right).
    pub margin_inline: Option<Length>,
    /// Vertical margin (top + bottom).
    pub margin_vertical: Option<Length>,
    /// Gap between children on both axes.
    pub gap: Option<Length>,
    /// Gap between rows.
    pub row_gap: Option<Length>,
    /// Gap between columns.
    pub column_gap: Option<Length>,
    /// Optional solid fill (engine white texture).
    pub background: Option<[f32; 4]>,
    /// Text color / image tint.
    pub color: [f32; 4],
    /// Image fit mode.
    pub fit: Fit,
    /// Image UV rect: min x, min y, max x, max y. Default is the full texture.
    pub uv: [f32; 4],
}

impl Default for Style {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            flex: None,
            flex_direction: None,
            align_items: None,
            justify_content: None,
            position: None,
            top: None,
            right: None,
            bottom: None,
            left: None,
            padding: None,
            padding_top: None,
            padding_right: None,
            padding_bottom: None,
            padding_left: None,
            padding_inline: None,
            padding_vertical: None,
            margin: None,
            margin_top: None,
            margin_right: None,
            margin_bottom: None,
            margin_left: None,
            margin_inline: None,
            margin_vertical: None,
            gap: None,
            row_gap: None,
            column_gap: None,
            background: None,
            color: [1.0, 1.0, 1.0, 1.0],
            fit: Fit::Fill,
            uv: [0.0, 0.0, 1.0, 1.0],
        }
    }
}

/// Build a [`Style`] with Yoga playground keys.
///
/// Lengths use `N pt`, `N %`, `ident pt`, or `(expr) pt`. Enums use short
/// names (`row`, `center`, `absolute`, `space_around`, …). Paint fields take
/// ordinary expressions (`background`, `color`, `fit`, `uv`).
#[macro_export]
macro_rules! style {
    () => {
        $crate::Style::default()
    };
    ($($rest:tt)*) => {{
        #[allow(unused_mut)]
        let mut __style = $crate::Style::default();
        $crate::__style_parse!(__style; $($rest)*);
        __style
    }};
}

/// Internal muncher for [`style!`].
#[doc(hidden)]
#[macro_export]
macro_rules! __style_parse {
    ($s:ident;) => {};
    ($s:ident; , $($rest:tt)*) => {
        $crate::__style_parse!($s; $($rest)*)
    };

    ($s:ident; width: $v:literal pt $($rest:tt)*) => {
        $s.width = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; width: $v:ident pt $($rest:tt)*) => {
        $s.width = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; width: ($v:expr) pt $($rest:tt)*) => {
        $s.width = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; width: $v:literal % $($rest:tt)*) => {
        $s.width = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; width: $v:ident % $($rest:tt)*) => {
        $s.width = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; width: ($v:expr) % $($rest:tt)*) => {
        $s.width = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; width: auto $($rest:tt)*) => {
        $s.width = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: $v:literal pt $($rest:tt)*) => {
        $s.height = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: $v:ident pt $($rest:tt)*) => {
        $s.height = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: ($v:expr) pt $($rest:tt)*) => {
        $s.height = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: $v:literal % $($rest:tt)*) => {
        $s.height = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: $v:ident % $($rest:tt)*) => {
        $s.height = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: ($v:expr) % $($rest:tt)*) => {
        $s.height = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; height: auto $($rest:tt)*) => {
        $s.height = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: $v:literal pt $($rest:tt)*) => {
        $s.top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: $v:ident pt $($rest:tt)*) => {
        $s.top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: ($v:expr) pt $($rest:tt)*) => {
        $s.top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: $v:literal % $($rest:tt)*) => {
        $s.top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: $v:ident % $($rest:tt)*) => {
        $s.top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: ($v:expr) % $($rest:tt)*) => {
        $s.top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; top: auto $($rest:tt)*) => {
        $s.top = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: $v:literal pt $($rest:tt)*) => {
        $s.right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: $v:ident pt $($rest:tt)*) => {
        $s.right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: ($v:expr) pt $($rest:tt)*) => {
        $s.right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: $v:literal % $($rest:tt)*) => {
        $s.right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: $v:ident % $($rest:tt)*) => {
        $s.right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: ($v:expr) % $($rest:tt)*) => {
        $s.right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; right: auto $($rest:tt)*) => {
        $s.right = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: $v:literal pt $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: $v:ident pt $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: ($v:expr) pt $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: $v:literal % $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: $v:ident % $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: ($v:expr) % $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; bottom: auto $($rest:tt)*) => {
        $s.bottom = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: $v:literal pt $($rest:tt)*) => {
        $s.left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: $v:ident pt $($rest:tt)*) => {
        $s.left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: ($v:expr) pt $($rest:tt)*) => {
        $s.left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: $v:literal % $($rest:tt)*) => {
        $s.left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: $v:ident % $($rest:tt)*) => {
        $s.left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: ($v:expr) % $($rest:tt)*) => {
        $s.left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; left: auto $($rest:tt)*) => {
        $s.left = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding: $v:literal pt $($rest:tt)*) => {
        $s.padding = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding: $v:ident pt $($rest:tt)*) => {
        $s.padding = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding: ($v:expr) pt $($rest:tt)*) => {
        $s.padding = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding: $v:literal % $($rest:tt)*) => {
        $s.padding = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding: $v:ident % $($rest:tt)*) => {
        $s.padding = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding: ($v:expr) % $($rest:tt)*) => {
        $s.padding = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_top: $v:literal pt $($rest:tt)*) => {
        $s.padding_top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_top: $v:ident pt $($rest:tt)*) => {
        $s.padding_top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_top: ($v:expr) pt $($rest:tt)*) => {
        $s.padding_top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_top: $v:literal % $($rest:tt)*) => {
        $s.padding_top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_top: $v:ident % $($rest:tt)*) => {
        $s.padding_top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_top: ($v:expr) % $($rest:tt)*) => {
        $s.padding_top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_right: $v:literal pt $($rest:tt)*) => {
        $s.padding_right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_right: $v:ident pt $($rest:tt)*) => {
        $s.padding_right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_right: ($v:expr) pt $($rest:tt)*) => {
        $s.padding_right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_right: $v:literal % $($rest:tt)*) => {
        $s.padding_right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_right: $v:ident % $($rest:tt)*) => {
        $s.padding_right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_right: ($v:expr) % $($rest:tt)*) => {
        $s.padding_right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_bottom: $v:literal pt $($rest:tt)*) => {
        $s.padding_bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_bottom: $v:ident pt $($rest:tt)*) => {
        $s.padding_bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_bottom: ($v:expr) pt $($rest:tt)*) => {
        $s.padding_bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_bottom: $v:literal % $($rest:tt)*) => {
        $s.padding_bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_bottom: $v:ident % $($rest:tt)*) => {
        $s.padding_bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_bottom: ($v:expr) % $($rest:tt)*) => {
        $s.padding_bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_left: $v:literal pt $($rest:tt)*) => {
        $s.padding_left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_left: $v:ident pt $($rest:tt)*) => {
        $s.padding_left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_left: ($v:expr) pt $($rest:tt)*) => {
        $s.padding_left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_left: $v:literal % $($rest:tt)*) => {
        $s.padding_left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_left: $v:ident % $($rest:tt)*) => {
        $s.padding_left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_left: ($v:expr) % $($rest:tt)*) => {
        $s.padding_left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_inline: $v:literal pt $($rest:tt)*) => {
        $s.padding_inline = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_inline: $v:ident pt $($rest:tt)*) => {
        $s.padding_inline = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_inline: ($v:expr) pt $($rest:tt)*) => {
        $s.padding_inline = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_inline: $v:literal % $($rest:tt)*) => {
        $s.padding_inline = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_inline: $v:ident % $($rest:tt)*) => {
        $s.padding_inline = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_inline: ($v:expr) % $($rest:tt)*) => {
        $s.padding_inline = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_vertical: $v:literal pt $($rest:tt)*) => {
        $s.padding_vertical = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_vertical: $v:ident pt $($rest:tt)*) => {
        $s.padding_vertical = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_vertical: ($v:expr) pt $($rest:tt)*) => {
        $s.padding_vertical = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_vertical: $v:literal % $($rest:tt)*) => {
        $s.padding_vertical = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_vertical: $v:ident % $($rest:tt)*) => {
        $s.padding_vertical = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; padding_vertical: ($v:expr) % $($rest:tt)*) => {
        $s.padding_vertical = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: $v:literal pt $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: $v:ident pt $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: ($v:expr) pt $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: $v:literal % $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: $v:ident % $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: ($v:expr) % $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin: auto $($rest:tt)*) => {
        $s.margin = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: $v:literal pt $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: $v:ident pt $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: ($v:expr) pt $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: $v:literal % $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: $v:ident % $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: ($v:expr) % $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_top: auto $($rest:tt)*) => {
        $s.margin_top = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: $v:literal pt $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: $v:ident pt $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: ($v:expr) pt $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: $v:literal % $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: $v:ident % $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: ($v:expr) % $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_right: auto $($rest:tt)*) => {
        $s.margin_right = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: $v:literal pt $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: $v:ident pt $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: ($v:expr) pt $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: $v:literal % $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: $v:ident % $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: ($v:expr) % $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_bottom: auto $($rest:tt)*) => {
        $s.margin_bottom = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: $v:literal pt $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: $v:ident pt $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: ($v:expr) pt $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: $v:literal % $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: $v:ident % $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: ($v:expr) % $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_left: auto $($rest:tt)*) => {
        $s.margin_left = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: $v:literal pt $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: $v:ident pt $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: ($v:expr) pt $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: $v:literal % $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: $v:ident % $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: ($v:expr) % $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_inline: auto $($rest:tt)*) => {
        $s.margin_inline = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: $v:literal pt $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: $v:ident pt $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: ($v:expr) pt $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: $v:literal % $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: $v:ident % $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: ($v:expr) % $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; margin_vertical: auto $($rest:tt)*) => {
        $s.margin_vertical = Some($crate::Length::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; gap: $v:literal pt $($rest:tt)*) => {
        $s.gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; gap: $v:ident pt $($rest:tt)*) => {
        $s.gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; gap: ($v:expr) pt $($rest:tt)*) => {
        $s.gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; gap: $v:literal % $($rest:tt)*) => {
        $s.gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; gap: $v:ident % $($rest:tt)*) => {
        $s.gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; gap: ($v:expr) % $($rest:tt)*) => {
        $s.gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; row_gap: $v:literal pt $($rest:tt)*) => {
        $s.row_gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; row_gap: $v:ident pt $($rest:tt)*) => {
        $s.row_gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; row_gap: ($v:expr) pt $($rest:tt)*) => {
        $s.row_gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; row_gap: $v:literal % $($rest:tt)*) => {
        $s.row_gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; row_gap: $v:ident % $($rest:tt)*) => {
        $s.row_gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; row_gap: ($v:expr) % $($rest:tt)*) => {
        $s.row_gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; column_gap: $v:literal pt $($rest:tt)*) => {
        $s.column_gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; column_gap: $v:ident pt $($rest:tt)*) => {
        $s.column_gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; column_gap: ($v:expr) pt $($rest:tt)*) => {
        $s.column_gap = Some($crate::Length::Pt($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; column_gap: $v:literal % $($rest:tt)*) => {
        $s.column_gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; column_gap: $v:ident % $($rest:tt)*) => {
        $s.column_gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; column_gap: ($v:expr) % $($rest:tt)*) => {
        $s.column_gap = Some($crate::Length::Pct($v as f32));
        $crate::__style_parse!($s; $($rest)*)
    };

    ($s:ident; flex: $v:literal $($rest:tt)*) => {
        $s.flex = Some($v as f32);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; flex: $v:ident $($rest:tt)*) => {
        $s.flex = Some($v as f32);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; flex: ($v:expr) $($rest:tt)*) => {
        $s.flex = Some($v as f32);
        $crate::__style_parse!($s; $($rest)*)
    };

    ($s:ident; flex_direction: column $($rest:tt)*) => {
        $s.flex_direction = Some($crate::FlexDirection::Column);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; flex_direction: column_reverse $($rest:tt)*) => {
        $s.flex_direction = Some($crate::FlexDirection::ColumnReverse);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; flex_direction: row $($rest:tt)*) => {
        $s.flex_direction = Some($crate::FlexDirection::Row);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; flex_direction: row_reverse $($rest:tt)*) => {
        $s.flex_direction = Some($crate::FlexDirection::RowReverse);
        $crate::__style_parse!($s; $($rest)*)
    };

    ($s:ident; align_items: auto $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: flex_start $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::FlexStart);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: center $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::Center);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: flex_end $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::FlexEnd);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: stretch $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::Stretch);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: baseline $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::Baseline);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: space_between $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::SpaceBetween);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: space_around $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::SpaceAround);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: space_evenly $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::SpaceEvenly);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: start $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::Start);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; align_items: end $($rest:tt)*) => {
        $s.align_items = Some($crate::Align::End);
        $crate::__style_parse!($s; $($rest)*)
    };

    ($s:ident; justify_content: auto $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::Auto);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: flex_start $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::FlexStart);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: center $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::Center);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: flex_end $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::FlexEnd);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: space_between $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::SpaceBetween);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: space_around $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::SpaceAround);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: space_evenly $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::SpaceEvenly);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: stretch $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::Stretch);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: start $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::Start);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; justify_content: end $($rest:tt)*) => {
        $s.justify_content = Some($crate::Justify::End);
        $crate::__style_parse!($s; $($rest)*)
    };

    ($s:ident; position: static $($rest:tt)*) => {
        $s.position = Some($crate::PositionType::Static);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; position: relative $($rest:tt)*) => {
        $s.position = Some($crate::PositionType::Relative);
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; position: absolute $($rest:tt)*) => {
        $s.position = Some($crate::PositionType::Absolute);
        $crate::__style_parse!($s; $($rest)*)
    };


    ($s:ident; background: $v:expr, $($rest:tt)*) => {
        $s.background = $v;
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; background: $v:expr) => {
        $s.background = $v;
    };
    ($s:ident; color: $v:expr, $($rest:tt)*) => {
        $s.color = $v;
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; color: $v:expr) => {
        $s.color = $v;
    };
    ($s:ident; fit: $v:expr, $($rest:tt)*) => {
        $s.fit = $v;
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; fit: $v:expr) => {
        $s.fit = $v;
    };
    ($s:ident; uv: $v:expr, $($rest:tt)*) => {
        $s.uv = $v;
        $crate::__style_parse!($s; $($rest)*)
    };
    ($s:ident; uv: $v:expr) => {
        $s.uv = $v;
    };
}
