//! Screen-fixed immediate-mode UI: boxes, images, and bitmap text.

use std::collections::HashSet;

use crate::core::math::Vec3;
use crate::core::render::TextureId;
use crate::core::scene::{Font, Sprite};

/// Stable id for hover and click queries across frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UiId(pub u32);

/// How children are laid out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Display {
    /// Stack children downward; stretch auto widths.
    #[default]
    Block,
    /// One-line flex along [`Style::direction`].
    Flex,
}

/// Main axis for flex layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    /// Left to right.
    #[default]
    Row,
    /// Top to bottom.
    Column,
}

/// Width or height.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Size {
    /// Content size (or stretch in block / flex cross when align is stretch).
    #[default]
    Auto,
    /// Fixed pixels.
    Px(f32),
    /// Fraction of the parent content box (`1.0` = 100%).
    Pct(f32),
    /// Share leftover main-axis space in a flex parent; stretch on the cross axis.
    Fill,
}

/// Main-axis placement of flex children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    /// Pack toward the start of the main axis.
    #[default]
    Start,
    /// Center along the main axis.
    Center,
    /// Pack toward the end of the main axis.
    End,
    /// Put free space between children.
    SpaceBetween,
}

/// Cross-axis placement of flex children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Align to the start of the cross axis.
    Start,
    /// Center on the cross axis.
    Center,
    /// Align to the end of the cross axis.
    End,
    /// Stretch `Auto` / `Fill` cross sizes to the line.
    #[default]
    Stretch,
}

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

/// Layout and paint for one node.
#[derive(Clone, Debug)]
pub struct Style {
    /// Block or flex.
    pub display: Display,
    /// Flex main axis.
    pub direction: Direction,
    /// Border-box width.
    pub width: Size,
    /// Border-box height.
    pub height: Size,
    /// Outside the border box: top, right, bottom, left.
    pub margin: [f32; 4],
    /// Inside the border box: top, right, bottom, left.
    pub padding: [f32; 4],
    /// Gap between children.
    pub gap: f32,
    /// Flex main-axis packing.
    pub justify: Justify,
    /// Flex cross-axis alignment.
    pub align: Align,
    /// Optional solid fill (uses the engine white texture).
    pub background: Option<[f32; 4]>,
    /// Text color / image tint.
    pub color: [f32; 4],
    /// Image fit mode.
    pub fit: Fit,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            display: Display::Block,
            direction: Direction::Row,
            width: Size::Auto,
            height: Size::Auto,
            margin: [0.0; 4],
            padding: [0.0; 4],
            gap: 0.0,
            justify: Justify::Start,
            align: Align::Stretch,
            background: None,
            color: [1.0, 1.0, 1.0, 1.0],
            fit: Fit::Fill,
        }
    }
}

impl Style {
    /// Same value on all four edges.
    pub fn edges(v: f32) -> [f32; 4] {
        [v, v, v, v]
    }
}

#[derive(Clone, Copy, Debug)]
struct Rect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl Rect {
    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.left && y >= self.top && x < self.left + self.width && y < self.top + self.height
    }

    fn right(self) -> f32 {
        self.left + self.width
    }

    fn bottom(self) -> f32 {
        self.top + self.height
    }
}

#[derive(Clone, Debug)]
enum Kind {
    Container,
    Image {
        texture: TextureId,
    },
    Text {
        font: Font,
        glyph_size: [f32; 2],
        text: String,
    },
}

#[derive(Clone, Debug)]
struct Node {
    id: UiId,
    style: Style,
    kind: Kind,
    children: Vec<usize>,
}

#[derive(Clone, Debug)]
struct HitEntry {
    id: UiId,
    rect: Rect,
    ancestors: Vec<UiId>,
}

/// Immediate-mode screen UI for the current frame.
#[derive(Debug, Default)]
pub struct Ui {
    nodes: Vec<Node>,
    stack: Vec<usize>,
    hovered: HashSet<UiId>,
    clicked: HashSet<UiId>,
    hit_rects: Vec<HitEntry>,
    sprites: Vec<Sprite>,
}

impl Ui {
    /// True if any node with this id was under the cursor last frame.
    pub fn hovered(&self, id: UiId) -> bool {
        self.hovered.contains(&id)
    }

    /// True if any node with this id was clicked last frame.
    pub fn clicked(&self, id: UiId) -> bool {
        self.clicked.contains(&id)
    }

    /// Start a new frame: clear the tree and hit-test last frame's boxes.
    pub(crate) fn begin_frame(
        &mut self,
        cursor: [f32; 2],
        cursor_inside: bool,
        pointer_pressed: bool,
    ) {
        self.nodes.clear();
        self.stack.clear();
        self.sprites.clear();
        self.hovered.clear();
        self.clicked.clear();

        if cursor_inside {
            let [x, y] = cursor;
            for entry in self.hit_rects.iter().rev() {
                if entry.rect.contains(x, y) {
                    self.hovered.insert(entry.id);
                    for &a in &entry.ancestors {
                        self.hovered.insert(a);
                    }
                    if pointer_pressed {
                        self.clicked.insert(entry.id);
                        for &a in &entry.ancestors {
                            self.clicked.insert(a);
                        }
                    }
                    break;
                }
            }
        }
        self.hit_rects.clear();
    }

    /// Add a container node and run `build` with it as the current parent.
    pub fn node(&mut self, id: UiId, style: Style, build: impl FnOnce(&mut Ui)) {
        let index = self.push_node(id, style, Kind::Container);
        self.stack.push(index);
        build(self);
        self.stack.pop();
    }

    /// Add an image node.
    pub fn image(&mut self, id: UiId, texture: TextureId, style: Style) {
        self.push_node(id, style, Kind::Image { texture });
    }

    /// Add a single-line bitmap text node.
    pub fn text(
        &mut self,
        id: UiId,
        font: &Font,
        glyph_size: [f32; 2],
        style: Style,
        text: &str,
    ) {
        self.push_node(
            id,
            style,
            Kind::Text {
                font: font.clone(),
                glyph_size,
                text: text.to_string(),
            },
        );
    }

    fn push_node(&mut self, id: UiId, style: Style, kind: Kind) -> usize {
        let index = self.nodes.len();
        self.nodes.push(Node {
            id,
            style,
            kind,
            children: Vec::new(),
        });
        if let Some(&parent) = self.stack.last() {
            self.nodes[parent].children.push(index);
        }
        index
    }

    /// Lay out the tree and emit sprites. `texture_size` returns pixel size or `(0,0)`.
    pub(crate) fn layout(
        &mut self,
        fb_size: [f32; 2],
        white: TextureId,
        mut texture_size: impl FnMut(TextureId) -> (u32, u32),
    ) {
        self.hit_rects.clear();
        self.sprites.clear();
        let roots: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| {
                !self
                    .nodes
                    .iter()
                    .any(|n| n.children.contains(&i))
            })
            .collect();

        let root_content = Rect {
            left: 0.0,
            top: 0.0,
            width: fb_size[0].max(1.0),
            height: fb_size[1].max(1.0),
        };

        let mut y = 0.0;
        for &root in &roots {
            let measured = measure_node(&self.nodes, root, Some(root_content.width), &mut texture_size);
            let stretch_w = matches!(self.nodes[root].style.display, Display::Block)
                && matches!(self.nodes[root].kind, Kind::Container);
            let w = resolve_axis(
                self.nodes[root].style.width,
                Some(root_content.width),
                measured.width,
                stretch_w,
            );
            let h = resolve_axis(
                self.nodes[root].style.height,
                Some(root_content.height),
                measured.height,
                false,
            );
            let style = &self.nodes[root].style;
            let left = root_content.left + style.margin[3];
            let top = y + style.margin[0];
            let border = Rect {
                left,
                top,
                width: w,
                height: h,
            };
            arrange_node(
                &self.nodes,
                root,
                border,
                &[],
                fb_size,
                white,
                &mut texture_size,
                &mut self.hit_rects,
                &mut self.sprites,
            );
            y = border.bottom() + style.margin[2];
        }
    }

    pub(crate) fn take_sprites(&mut self) -> Vec<Sprite> {
        std::mem::take(&mut self.sprites)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Measured {
    /// Border-box preferred width (content + padding), ignoring margin.
    width: f32,
    /// Border-box preferred height.
    height: f32,
}

fn pad_x(style: &Style) -> f32 {
    style.padding[1] + style.padding[3]
}

fn pad_y(style: &Style) -> f32 {
    style.padding[0] + style.padding[2]
}

fn resolve_axis(size: Size, parent: Option<f32>, intrinsic: f32, stretch_auto: bool) -> f32 {
    match size {
        Size::Px(v) => v.max(0.0),
        Size::Pct(p) => match parent {
            Some(parent) if parent.is_finite() && parent > 0.0 => (parent * p).max(0.0),
            _ => intrinsic.max(0.0),
        },
        Size::Fill => match parent {
            Some(parent) => parent.max(0.0),
            None => intrinsic.max(0.0),
        },
        Size::Auto => {
            if stretch_auto {
                match parent {
                    Some(parent) => parent.max(0.0),
                    None => intrinsic.max(0.0),
                }
            } else {
                intrinsic.max(0.0)
            }
        }
    }
}

fn measure_node(
    nodes: &[Node],
    index: usize,
    parent_content_w: Option<f32>,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
) -> Measured {
    let node = &nodes[index];
    let style = &node.style;

    let content_intrinsic = match &node.kind {
        Kind::Text {
            font,
            glyph_size,
            text,
        } => {
            let size = font.measure(*glyph_size, text);
            Measured {
                width: size[0],
                height: size[1],
            }
        }
        Kind::Image { texture } => {
            let (tw, th) = texture_size(*texture);
            Measured {
                width: tw as f32,
                height: th as f32,
            }
        }
        Kind::Container => measure_children(nodes, index, parent_content_w, texture_size),
    };

    let mut width = match style.width {
        Size::Px(v) => v.max(0.0),
        Size::Pct(p) => match parent_content_w {
            Some(pw) if pw > 0.0 => (pw * p).max(0.0),
            _ => content_intrinsic.width + pad_x(style),
        },
        Size::Fill | Size::Auto => content_intrinsic.width + pad_x(style),
    };
    let mut height = match style.height {
        Size::Px(v) => v.max(0.0),
        // Percent height needs a definite parent height; measure only has width.
        Size::Pct(_p) => content_intrinsic.height + pad_y(style),
        Size::Fill | Size::Auto => content_intrinsic.height + pad_y(style),
    };

    // When width/height are definite from Px, still ensure padding fits.
    width = width.max(pad_x(style));
    height = height.max(pad_y(style));

    Measured { width, height }
}

fn measure_children(
    nodes: &[Node],
    index: usize,
    parent_content_w: Option<f32>,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
) -> Measured {
    let node = &nodes[index];
    let style = &node.style;
    let child_parent_w = match style.width {
        Size::Px(v) => Some((v - pad_x(style)).max(0.0)),
        Size::Pct(p) => parent_content_w.map(|pw| (pw * p - pad_x(style)).max(0.0)),
        Size::Auto | Size::Fill => parent_content_w.map(|pw| (pw - pad_x(style)).max(0.0)),
    };

    if node.children.is_empty() {
        return Measured::default();
    }

    match style.display {
        Display::Block => {
            let mut w = 0.0f32;
            let mut h = 0.0f32;
            let n = node.children.len();
            for (i, &child) in node.children.iter().enumerate() {
                let m = measure_node(nodes, child, child_parent_w, texture_size);
                let cs = &nodes[child].style;
                let child_w = m.width + cs.margin[1] + cs.margin[3];
                let child_h = m.height + cs.margin[0] + cs.margin[2];
                w = w.max(child_w);
                h += child_h;
                if i + 1 < n {
                    h += style.gap;
                }
            }
            Measured { width: w, height: h }
        }
        Display::Flex => match style.direction {
            Direction::Row => {
                let mut w = 0.0f32;
                let mut h = 0.0f32;
                let n = node.children.len();
                for (i, &child) in node.children.iter().enumerate() {
                    let m = measure_node(nodes, child, child_parent_w, texture_size);
                    let cs = &nodes[child].style;
                    w += m.width + cs.margin[1] + cs.margin[3];
                    h = h.max(m.height + cs.margin[0] + cs.margin[2]);
                    if i + 1 < n {
                        w += style.gap;
                    }
                }
                Measured { width: w, height: h }
            }
            Direction::Column => {
                let mut w = 0.0f32;
                let mut h = 0.0f32;
                let n = node.children.len();
                for (i, &child) in node.children.iter().enumerate() {
                    let m = measure_node(nodes, child, child_parent_w, texture_size);
                    let cs = &nodes[child].style;
                    w = w.max(m.width + cs.margin[1] + cs.margin[3]);
                    h += m.height + cs.margin[0] + cs.margin[2];
                    if i + 1 < n {
                        h += style.gap;
                    }
                }
                Measured { width: w, height: h }
            }
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn arrange_node(
    nodes: &[Node],
    index: usize,
    border: Rect,
    ancestors: &[UiId],
    fb_size: [f32; 2],
    white: TextureId,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
    hits: &mut Vec<HitEntry>,
    sprites: &mut Vec<Sprite>,
) {
    let node = &nodes[index];
    let style = &node.style;

    hits.push(HitEntry {
        id: node.id,
        rect: border,
        ancestors: ancestors.to_vec(),
    });

    if let Some(bg) = style.background {
        push_rect_sprite(sprites, white, border, fb_size, bg);
    }

    let content = Rect {
        left: border.left + style.padding[3],
        top: border.top + style.padding[0],
        width: (border.width - pad_x(style)).max(0.0),
        height: (border.height - pad_y(style)).max(0.0),
    };

    match &node.kind {
        Kind::Image { texture } => {
            let (tw, th) = texture_size(*texture);
            let dest = fit_rect(content, tw as f32, th as f32, style.fit);
            push_rect_sprite(sprites, *texture, dest, fb_size, style.color);
        }
        Kind::Text {
            font,
            glyph_size,
            text,
        } => {
            push_text_sprites(
                sprites,
                font,
                *glyph_size,
                style.color,
                text,
                content.left,
                content.top + content.height * 0.5,
                fb_size,
            );
        }
        Kind::Container => {
            let mut next_ancestors = ancestors.to_vec();
            next_ancestors.push(node.id);
            arrange_children(
                nodes,
                index,
                content,
                &next_ancestors,
                fb_size,
                white,
                texture_size,
                hits,
                sprites,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn arrange_children(
    nodes: &[Node],
    index: usize,
    content: Rect,
    ancestors: &[UiId],
    fb_size: [f32; 2],
    white: TextureId,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
    hits: &mut Vec<HitEntry>,
    sprites: &mut Vec<Sprite>,
) {
    let node = &nodes[index];
    let style = &node.style;
    if node.children.is_empty() {
        return;
    }

    match style.display {
        Display::Block => {
            let mut y = content.top;
            let n = node.children.len();
            for (i, &child) in node.children.iter().enumerate() {
                let cs = &nodes[child].style;
                let measured = measure_node(nodes, child, Some(content.width), texture_size);
                let avail_w = (content.width - cs.margin[1] - cs.margin[3]).max(0.0);
                let w = resolve_axis(cs.width, Some(avail_w), measured.width, true);
                let h = resolve_axis(cs.height, None, measured.height, false);
                let border = Rect {
                    left: content.left + cs.margin[3],
                    top: y + cs.margin[0],
                    width: w,
                    height: h,
                };
                arrange_node(
                    nodes,
                    child,
                    border,
                    ancestors,
                    fb_size,
                    white,
                    texture_size,
                    hits,
                    sprites,
                );
                y = border.bottom() + cs.margin[2];
                if i + 1 < n {
                    y += style.gap;
                }
            }
        }
        Display::Flex => match style.direction {
            Direction::Row => arrange_flex_row(
                nodes,
                index,
                content,
                ancestors,
                fb_size,
                white,
                texture_size,
                hits,
                sprites,
            ),
            Direction::Column => arrange_flex_column(
                nodes,
                index,
                content,
                ancestors,
                fb_size,
                white,
                texture_size,
                hits,
                sprites,
            ),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn arrange_flex_row(
    nodes: &[Node],
    index: usize,
    content: Rect,
    ancestors: &[UiId],
    fb_size: [f32; 2],
    white: TextureId,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
    hits: &mut Vec<HitEntry>,
    sprites: &mut Vec<Sprite>,
) {
    let node = &nodes[index];
    let style = &node.style;
    let n = node.children.len();
    let mut sizes: Vec<(f32, f32)> = Vec::with_capacity(n);
    let mut margins: Vec<[f32; 4]> = Vec::with_capacity(n);
    let mut fill_count = 0usize;
    let mut fixed_main = 0.0f32;

    for &child in &node.children {
        let cs = &nodes[child].style;
        let measured = measure_node(nodes, child, Some(content.width), texture_size);
        let is_fill = matches!(cs.width, Size::Fill);
        let w = if is_fill {
            fill_count += 1;
            measured.width
        } else {
            resolve_axis(cs.width, Some(content.width), measured.width, false)
        };
        let stretch = matches!(style.align, Align::Stretch)
            && matches!(cs.height, Size::Auto | Size::Fill);
        let avail_h = (content.height - cs.margin[0] - cs.margin[2]).max(0.0);
        let h = if stretch {
            avail_h
        } else {
            resolve_axis(cs.height, Some(avail_h), measured.height, false)
        };
        let margin_main = cs.margin[1] + cs.margin[3];
        if !is_fill {
            fixed_main += w + margin_main;
        } else {
            fixed_main += margin_main;
        }
        sizes.push((w, h));
        margins.push(cs.margin);
    }

    let gaps = if n > 1 { style.gap * (n - 1) as f32 } else { 0.0 };
    let free = (content.width - fixed_main - gaps).max(0.0);
    let fill_each = if fill_count > 0 {
        free / fill_count as f32
    } else {
        0.0
    };

    for (i, &child) in node.children.iter().enumerate() {
        if matches!(nodes[child].style.width, Size::Fill) {
            sizes[i].0 = fill_each.max(0.0);
        }
    }

    let used_main: f32 = sizes
        .iter()
        .zip(margins.iter())
        .map(|((w, _), m)| w + m[1] + m[3])
        .sum::<f32>()
        + gaps;

    let (mut x, justify_gap) = match style.justify {
        Justify::Start => (content.left, 0.0),
        Justify::Center => (content.left + (content.width - used_main) * 0.5, 0.0),
        Justify::End => (content.left + content.width - used_main, 0.0),
        Justify::SpaceBetween => {
            if n > 1 {
                let extra = (content.width - (used_main - gaps)).max(0.0);
                (content.left, extra / (n - 1) as f32)
            } else {
                (content.left, 0.0)
            }
        }
    };

    for (i, &child) in node.children.iter().enumerate() {
        let (w, h) = sizes[i];
        let m = margins[i];
        let cross = content.height - m[0] - m[2];
        let top = match style.align {
            Align::Start | Align::Stretch => content.top + m[0],
            Align::Center => content.top + m[0] + (cross - h) * 0.5,
            Align::End => content.top + m[0] + (cross - h).max(0.0),
        };
        let border = Rect {
            left: x + m[3],
            top,
            width: w,
            height: h,
        };
        arrange_node(
            nodes,
            child,
            border,
            ancestors,
            fb_size,
            white,
            texture_size,
            hits,
            sprites,
        );
        x = border.right() + m[1];
        if i + 1 < n {
            x += if matches!(style.justify, Justify::SpaceBetween) {
                justify_gap
            } else {
                style.gap
            };
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn arrange_flex_column(
    nodes: &[Node],
    index: usize,
    content: Rect,
    ancestors: &[UiId],
    fb_size: [f32; 2],
    white: TextureId,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
    hits: &mut Vec<HitEntry>,
    sprites: &mut Vec<Sprite>,
) {
    let node = &nodes[index];
    let style = &node.style;
    let n = node.children.len();
    let mut sizes: Vec<(f32, f32)> = Vec::with_capacity(n);
    let mut margins: Vec<[f32; 4]> = Vec::with_capacity(n);
    let mut fill_count = 0usize;
    let mut fixed_main = 0.0f32;

    for &child in &node.children {
        let cs = &nodes[child].style;
        let measured = measure_node(nodes, child, Some(content.width), texture_size);
        let is_fill = matches!(cs.height, Size::Fill);
        let h = if is_fill {
            fill_count += 1;
            measured.height
        } else {
            resolve_axis(cs.height, Some(content.height), measured.height, false)
        };
        let stretch = matches!(style.align, Align::Stretch)
            && matches!(cs.width, Size::Auto | Size::Fill);
        let avail_w = (content.width - cs.margin[1] - cs.margin[3]).max(0.0);
        let w = if stretch {
            avail_w
        } else {
            resolve_axis(cs.width, Some(avail_w), measured.width, false)
        };
        let margin_main = cs.margin[0] + cs.margin[2];
        if !is_fill {
            fixed_main += h + margin_main;
        } else {
            fixed_main += margin_main;
        }
        sizes.push((w, h));
        margins.push(cs.margin);
    }

    let gaps = if n > 1 { style.gap * (n - 1) as f32 } else { 0.0 };
    let free = (content.height - fixed_main - gaps).max(0.0);
    let fill_each = if fill_count > 0 {
        free / fill_count as f32
    } else {
        0.0
    };

    for (i, &child) in node.children.iter().enumerate() {
        if matches!(nodes[child].style.height, Size::Fill) {
            sizes[i].1 = fill_each.max(0.0);
        }
    }

    let used_main: f32 = sizes
        .iter()
        .zip(margins.iter())
        .map(|((_, h), m)| h + m[0] + m[2])
        .sum::<f32>()
        + gaps;

    let (mut y, justify_gap) = match style.justify {
        Justify::Start => (content.top, 0.0),
        Justify::Center => (content.top + (content.height - used_main) * 0.5, 0.0),
        Justify::End => (content.top + content.height - used_main, 0.0),
        Justify::SpaceBetween => {
            if n > 1 {
                let extra = (content.height - (used_main - gaps)).max(0.0);
                (content.top, extra / (n - 1) as f32)
            } else {
                (content.top, 0.0)
            }
        }
    };

    for (i, &child) in node.children.iter().enumerate() {
        let (w, h) = sizes[i];
        let m = margins[i];
        let cross = content.width - m[1] - m[3];
        let left = match style.align {
            Align::Start | Align::Stretch => content.left + m[3],
            Align::Center => content.left + m[3] + (cross - w) * 0.5,
            Align::End => content.left + m[3] + (cross - w).max(0.0),
        };
        let border = Rect {
            left,
            top: y + m[0],
            width: w,
            height: h,
        };
        arrange_node(
            nodes,
            child,
            border,
            ancestors,
            fb_size,
            white,
            texture_size,
            hits,
            sprites,
        );
        y = border.bottom() + m[2];
        if i + 1 < n {
            y += if matches!(style.justify, Justify::SpaceBetween) {
                justify_gap
            } else {
                style.gap
            };
        }
    }
}

fn fit_rect(content: Rect, tw: f32, th: f32, fit: Fit) -> Rect {
    if tw <= 0.0 || th <= 0.0 {
        return content;
    }
    match fit {
        Fit::Fill => content,
        Fit::Contain => {
            let sx = content.width / tw;
            let sy = content.height / th;
            let s = sx.min(sy);
            let w = tw * s;
            let h = th * s;
            Rect {
                left: content.left + (content.width - w) * 0.5,
                top: content.top + (content.height - h) * 0.5,
                width: w,
                height: h,
            }
        }
        Fit::Cover => {
            let sx = content.width / tw;
            let sy = content.height / th;
            let s = sx.max(sy);
            let w = tw * s;
            let h = th * s;
            Rect {
                left: content.left + (content.width - w) * 0.5,
                top: content.top + (content.height - h) * 0.5,
                width: w,
                height: h,
            }
        }
    }
}

fn screen_to_ui_center(rect: Rect, fb_size: [f32; 2]) -> Vec3 {
    let cx = rect.left + rect.width * 0.5 - fb_size[0] * 0.5;
    let cy = fb_size[1] * 0.5 - (rect.top + rect.height * 0.5);
    Vec3::new(cx, cy, 0.0)
}

fn push_rect_sprite(
    sprites: &mut Vec<Sprite>,
    texture: TextureId,
    rect: Rect,
    fb_size: [f32; 2],
    color: [f32; 4],
) {
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let mut s = Sprite::new(
        texture,
        screen_to_ui_center(rect, fb_size),
        [rect.width, rect.height],
    );
    s.color = color;
    sprites.push(s);
}

fn push_text_sprites(
    sprites: &mut Vec<Sprite>,
    font: &Font,
    glyph_size: [f32; 2],
    color: [f32; 4],
    text: &str,
    origin_x: f32,
    center_y_screen: f32,
    fb_size: [f32; 2],
) {
    let top = center_y_screen - glyph_size[1] * 0.5;
    font.emit_glyphs_y_down(text, glyph_size, |uv, offset, size| {
        let rect = Rect {
            left: origin_x + offset[0],
            top: top + offset[1],
            width: size[0],
            height: size[1],
        };
        let mut s = Sprite::new(
            font.texture,
            screen_to_ui_center(rect, fb_size),
            size,
        );
        s.uv = uv;
        s.color = color;
        sprites.push(s);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_size(_: TextureId) -> (u32, u32) {
        (0, 0)
    }

    fn layout_hits(ui: &mut Ui, fb: [f32; 2]) -> Vec<(UiId, Rect)> {
        ui.layout(fb, TextureId::from_raw(0), empty_size);
        ui.hit_rects
            .iter()
            .map(|h| (h.id, h.rect))
            .collect()
    }

    #[test]
    fn block_stacks_children() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            Style {
                width: Size::Px(100.0),
                height: Size::Auto,
                ..Style::default()
            },
            |ui| {
                ui.node(
                    UiId(2),
                    Style {
                        height: Size::Px(20.0),
                        ..Style::default()
                    },
                    |_| {},
                );
                ui.node(
                    UiId(3),
                    Style {
                        height: Size::Px(30.0),
                        ..Style::default()
                    },
                    |_| {},
                );
            },
        );
        let hits = layout_hits(&mut ui, [200.0, 200.0]);
        let a = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        let b = hits.iter().find(|(id, _)| *id == UiId(3)).unwrap().1;
        assert!((a.top - 0.0).abs() < 1e-3);
        assert!((b.top - 20.0).abs() < 1e-3);
        assert!((a.width - 100.0).abs() < 1e-3);
    }

    #[test]
    fn flex_row_fill_shares_space() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            Style {
                display: Display::Flex,
                direction: Direction::Row,
                width: Size::Px(100.0),
                height: Size::Px(40.0),
                ..Style::default()
            },
            |ui| {
                ui.node(
                    UiId(2),
                    Style {
                        width: Size::Fill,
                        height: Size::Px(40.0),
                        ..Style::default()
                    },
                    |_| {},
                );
                ui.node(
                    UiId(3),
                    Style {
                        width: Size::Fill,
                        height: Size::Px(40.0),
                        ..Style::default()
                    },
                    |_| {},
                );
            },
        );
        let hits = layout_hits(&mut ui, [200.0, 200.0]);
        let a = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        let b = hits.iter().find(|(id, _)| *id == UiId(3)).unwrap().1;
        assert!((a.width - 50.0).abs() < 1e-3);
        assert!((b.width - 50.0).abs() < 1e-3);
        assert!((b.left - 50.0).abs() < 1e-3);
    }

    #[test]
    fn percent_width_of_parent() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            Style {
                width: Size::Px(200.0),
                height: Size::Px(50.0),
                ..Style::default()
            },
            |ui| {
                ui.node(
                    UiId(2),
                    Style {
                        width: Size::Pct(0.5),
                        height: Size::Px(20.0),
                        ..Style::default()
                    },
                    |_| {},
                );
            },
        );
        let hits = layout_hits(&mut ui, [400.0, 400.0]);
        let child = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        assert!((child.width - 100.0).abs() < 1e-3);
    }

    #[test]
    fn text_width_is_glyph_count() {
        let font = Font::grid(TextureId::from_raw(1), 10, 1, '0');
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.text(
            UiId(1),
            &font,
            [10.0, 16.0],
            Style::default(),
            "123",
        );
        let hits = layout_hits(&mut ui, [200.0, 200.0]);
        let r = hits.iter().find(|(id, _)| *id == UiId(1)).unwrap().1;
        assert!((r.width - 30.0).abs() < 1e-3);
        assert!((r.height - 16.0).abs() < 1e-3);
    }

    #[test]
    fn image_contain_keeps_aspect() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.image(
            UiId(1),
            TextureId::from_raw(5),
            Style {
                width: Size::Px(100.0),
                height: Size::Px(50.0),
                fit: Fit::Contain,
                ..Style::default()
            },
        );
        ui.layout([200.0, 200.0], TextureId::from_raw(0), |id| {
            if id == TextureId::from_raw(5) {
                (40, 40)
            } else {
                (0, 0)
            }
        });
        assert_eq!(ui.sprites.len(), 1);
        let s = &ui.sprites[0];
        assert!((s.size[0] - 50.0).abs() < 1e-3);
        assert!((s.size[1] - 50.0).abs() < 1e-3);
    }

    #[test]
    fn hit_test_topmost_and_ancestors() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            Style {
                width: Size::Px(100.0),
                height: Size::Px(100.0),
                ..Style::default()
            },
            |ui| {
                ui.node(
                    UiId(2),
                    Style {
                        width: Size::Px(40.0),
                        height: Size::Px(40.0),
                        ..Style::default()
                    },
                    |_| {},
                );
            },
        );
        ui.layout([200.0, 200.0], TextureId::from_raw(0), empty_size);
        ui.begin_frame([10.0, 10.0], true, true);
        assert!(ui.hovered(UiId(2)));
        assert!(ui.hovered(UiId(1)));
        assert!(ui.clicked(UiId(2)));
        assert!(ui.clicked(UiId(1)));
    }

    #[test]
    fn later_sibling_wins_hit() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            Style {
                width: Size::Px(50.0),
                height: Size::Px(50.0),
                ..Style::default()
            },
            |_| {},
        );
        // Second top-level node starts below the first in block flow under the root.
        // Overlap by placing both with absolute-ish sizing via nested empty roots:
        // Simulate overlap by laying out two full-size overlapping panels as siblings
        // under a flex with zero gap and Fill — use padding-free full rects via
        // rebuilding with known hit rects manually isn't possible; instead nest
        // both at top-left by using a flex row with negative-free space isn't allowed.
        // Place a second panel as child that covers the first via a later sibling in
        // the same parent with margin that keeps it on top in paint order at same spot.
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(10),
            Style {
                display: Display::Flex,
                direction: Direction::Row,
                width: Size::Px(50.0),
                height: Size::Px(50.0),
                ..Style::default()
            },
            |ui| {
                ui.node(
                    UiId(11),
                    Style {
                        width: Size::Px(50.0),
                        height: Size::Px(50.0),
                        ..Style::default()
                    },
                    |_| {},
                );
            },
        );
        ui.node(
            UiId(20),
            Style {
                width: Size::Px(50.0),
                height: Size::Px(50.0),
                margin: [-50.0, 0.0, 0.0, 0.0],
                ..Style::default()
            },
            |_| {},
        );
        ui.layout([200.0, 200.0], TextureId::from_raw(0), empty_size);
        ui.begin_frame([25.0, 25.0], true, false);
        assert!(ui.hovered(UiId(20)));
        assert!(!ui.hovered(UiId(11)));
    }
}
