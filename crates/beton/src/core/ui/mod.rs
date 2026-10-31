//! Screen-fixed immediate-mode UI: boxes, images, and bitmap text.
//!
//! Layout is [Yoga](https://www.yogalayout.dev/) via the pure-Rust `yoga-rs` crate.

mod style;

pub use style::{Fit, Length, Style};

use std::collections::HashSet;

use yoga::{
    Align, BoxSizing, Direction as YogaDirection, Edge, FlexDirection, Gutter, NodeRef,
    Size as YogaSize,
};

use crate::core::math::Vec3;
use crate::core::render::TextureId;
use crate::core::scene::{Font, Sprite};

/// Stable id for hover and click queries across frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UiId(pub u32);

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

    /// Lay out the tree with Yoga and emit sprites. `texture_size` returns pixel size or `(0,0)`.
    pub(crate) fn layout(
        &mut self,
        fb_size: [f32; 2],
        white: TextureId,
        mut texture_size: impl FnMut(TextureId) -> (u32, u32),
    ) {
        self.hit_rects.clear();
        self.sprites.clear();
        if self.nodes.is_empty() {
            return;
        }

        let roots: Vec<usize> = (0..self.nodes.len())
            .filter(|&i| !self.nodes.iter().any(|n| n.children.contains(&i)))
            .collect();

        let cfg = yoga::config::new_config();
        cfg.borrow_mut().set_use_web_defaults(false);
        cfg.borrow_mut().set_point_scale_factor(0.0);

        let root = NodeRef::with_config(cfg.clone());
        root.set_style_box_sizing(BoxSizing::BorderBox);
        root.set_style_flex_direction(FlexDirection::Column);
        root.set_style_align_items(Align::FlexStart);
        root.set_style_width(fb_size[0].max(1.0));
        root.set_style_height(fb_size[1].max(1.0));

        // Keep NodeRefs alive for the layout pass.
        let mut yoga_nodes: Vec<Option<NodeRef>> = vec![None; self.nodes.len()];

        for (i, &idx) in roots.iter().enumerate() {
            let yn = build_yoga_node(&self.nodes, idx, &cfg, &mut texture_size, &mut yoga_nodes);
            root.insert_child(&yn, i);
            yoga_nodes[idx] = Some(yn);
        }

        root.calculate_layout(
            fb_size[0].max(1.0),
            fb_size[1].max(1.0),
            YogaDirection::LTR,
        );

        for &idx in &roots {
            let Some(yn) = yoga_nodes[idx].as_ref() else {
                continue;
            };
            emit_node(
                &self.nodes,
                idx,
                yn,
                0.0,
                0.0,
                &[],
                fb_size,
                white,
                &mut texture_size,
                &mut self.hit_rects,
                &mut self.sprites,
                &yoga_nodes,
            );
        }
    }

    pub(crate) fn take_sprites(&mut self) -> Vec<Sprite> {
        std::mem::take(&mut self.sprites)
    }
}

fn build_yoga_node(
    nodes: &[Node],
    index: usize,
    cfg: &yoga::ConfigRef,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
    yoga_nodes: &mut [Option<NodeRef>],
) -> NodeRef {
    let node = &nodes[index];
    let yn = NodeRef::with_config(cfg.clone());
    yn.set_style_box_sizing(BoxSizing::BorderBox);
    apply_style(&yn, &node.style);

    match &node.kind {
        Kind::Text {
            font,
            glyph_size,
            text,
        } => {
            let size = font.measure(*glyph_size, text);
            let w = size[0];
            let h = size[1];
            yn.set_measure_fn(move |_n, _aw, _wm, _ah, _hm| YogaSize {
                width: w,
                height: h,
            });
        }
        Kind::Image { texture } => {
            let (tw, th) = texture_size(*texture);
            let uv = node.style.uv;
            let cell_w = tw as f32 * (uv[2] - uv[0]).abs().max(0.0);
            let cell_h = th as f32 * (uv[3] - uv[1]).abs().max(0.0);
            yn.set_measure_fn(move |_n, _aw, _wm, _ah, _hm| YogaSize {
                width: cell_w,
                height: cell_h,
            });
        }
        Kind::Container => {
            for (i, &child) in node.children.iter().enumerate() {
                let child_yn =
                    build_yoga_node(nodes, child, cfg, texture_size, yoga_nodes);
                yn.insert_child(&child_yn, i);
                yoga_nodes[child] = Some(child_yn);
            }
        }
    }

    yn
}

fn apply_style(yn: &NodeRef, style: &Style) {
    if let Some(w) = style.width {
        apply_dim_width(yn, w);
    }
    if let Some(h) = style.height {
        apply_dim_height(yn, h);
    }
    if let Some(flex) = style.flex {
        yn.set_style_flex(flex);
    }
    if let Some(dir) = style.flex_direction {
        yn.set_style_flex_direction(dir);
    }
    if let Some(align) = style.align_items {
        yn.set_style_align_items(align);
    }
    if let Some(justify) = style.justify_content {
        yn.set_style_justify_content(justify);
    }
    if let Some(pos) = style.position {
        yn.set_style_position_type(pos);
    }
    apply_inset(yn, Edge::Top, style.top);
    apply_inset(yn, Edge::Right, style.right);
    apply_inset(yn, Edge::Bottom, style.bottom);
    apply_inset(yn, Edge::Left, style.left);

    apply_edge(yn, Edge::All, style.padding, EdgeKind::Padding);
    apply_edge(yn, Edge::Top, style.padding_top, EdgeKind::Padding);
    apply_edge(yn, Edge::Right, style.padding_right, EdgeKind::Padding);
    apply_edge(yn, Edge::Bottom, style.padding_bottom, EdgeKind::Padding);
    apply_edge(yn, Edge::Left, style.padding_left, EdgeKind::Padding);
    apply_edge(yn, Edge::Horizontal, style.padding_inline, EdgeKind::Padding);
    apply_edge(yn, Edge::Vertical, style.padding_vertical, EdgeKind::Padding);

    apply_edge(yn, Edge::All, style.margin, EdgeKind::Margin);
    apply_edge(yn, Edge::Top, style.margin_top, EdgeKind::Margin);
    apply_edge(yn, Edge::Right, style.margin_right, EdgeKind::Margin);
    apply_edge(yn, Edge::Bottom, style.margin_bottom, EdgeKind::Margin);
    apply_edge(yn, Edge::Left, style.margin_left, EdgeKind::Margin);
    apply_edge(yn, Edge::Horizontal, style.margin_inline, EdgeKind::Margin);
    apply_edge(yn, Edge::Vertical, style.margin_vertical, EdgeKind::Margin);

    apply_gap(yn, Gutter::All, style.gap);
    apply_gap(yn, Gutter::Row, style.row_gap);
    apply_gap(yn, Gutter::Column, style.column_gap);
}

fn apply_dim_width(yn: &NodeRef, len: Length) {
    match len {
        Length::Pt(v) => yn.set_style_width(v.max(0.0)),
        Length::Pct(v) => yn.set_style_width_percent(v),
        Length::Auto => yn.set_style_width_auto(),
    }
}

fn apply_dim_height(yn: &NodeRef, len: Length) {
    match len {
        Length::Pt(v) => yn.set_style_height(v.max(0.0)),
        Length::Pct(v) => yn.set_style_height_percent(v),
        Length::Auto => yn.set_style_height_auto(),
    }
}

fn apply_inset(yn: &NodeRef, edge: Edge, len: Option<Length>) {
    let Some(len) = len else {
        return;
    };
    match len {
        Length::Pt(v) => yn.set_style_position(edge, v),
        Length::Pct(v) => yn.set_style_position_percent(edge, v),
        Length::Auto => yn.set_style_position_auto(edge),
    }
}

#[derive(Clone, Copy)]
enum EdgeKind {
    Padding,
    Margin,
}

fn apply_edge(yn: &NodeRef, edge: Edge, len: Option<Length>, kind: EdgeKind) {
    let Some(len) = len else {
        return;
    };
    match (kind, len) {
        (EdgeKind::Padding, Length::Pt(v)) => yn.set_style_padding(edge, v.max(0.0)),
        (EdgeKind::Padding, Length::Pct(v)) => yn.set_style_padding_percent(edge, v),
        (EdgeKind::Padding, Length::Auto) => {}
        (EdgeKind::Margin, Length::Pt(v)) => yn.set_style_margin(edge, v),
        (EdgeKind::Margin, Length::Pct(v)) => yn.set_style_margin_percent(edge, v),
        (EdgeKind::Margin, Length::Auto) => yn.set_style_margin_auto(edge),
    }
}

fn apply_gap(yn: &NodeRef, gutter: Gutter, len: Option<Length>) {
    let Some(len) = len else {
        return;
    };
    match len {
        Length::Pt(v) => yn.set_style_gap(gutter, v.max(0.0)),
        Length::Pct(v) => yn.set_style_gap_percent(gutter, v),
        Length::Auto => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_node(
    nodes: &[Node],
    index: usize,
    yn: &NodeRef,
    parent_left: f32,
    parent_top: f32,
    ancestors: &[UiId],
    fb_size: [f32; 2],
    white: TextureId,
    texture_size: &mut impl FnMut(TextureId) -> (u32, u32),
    hits: &mut Vec<HitEntry>,
    sprites: &mut Vec<Sprite>,
    yoga_nodes: &[Option<NodeRef>],
) {
    let node = &nodes[index];
    let border = Rect {
        left: parent_left + yn.layout_left(),
        top: parent_top + yn.layout_top(),
        width: yn.layout_width().max(0.0),
        height: yn.layout_height().max(0.0),
    };

    hits.push(HitEntry {
        id: node.id,
        rect: border,
        ancestors: ancestors.to_vec(),
    });

    if let Some(bg) = node.style.background {
        push_rect_sprite(sprites, white, border, fb_size, bg, [0.0, 0.0, 1.0, 1.0]);
    }

    let pad_l = yn.layout_padding(Edge::Left);
    let pad_t = yn.layout_padding(Edge::Top);
    let pad_r = yn.layout_padding(Edge::Right);
    let pad_b = yn.layout_padding(Edge::Bottom);
    let content = Rect {
        left: border.left + pad_l,
        top: border.top + pad_t,
        width: (border.width - pad_l - pad_r).max(0.0),
        height: (border.height - pad_t - pad_b).max(0.0),
    };

    match &node.kind {
        Kind::Image { texture } => {
            let (tw, th) = texture_size(*texture);
            let uv = node.style.uv;
            let cell_w = tw as f32 * (uv[2] - uv[0]).abs().max(0.0);
            let cell_h = th as f32 * (uv[3] - uv[1]).abs().max(0.0);
            let dest = fit_rect(content, cell_w, cell_h, node.style.fit);
            push_rect_sprite(sprites, *texture, dest, fb_size, node.style.color, uv);
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
                node.style.color,
                text,
                content.left,
                content.top + content.height * 0.5,
                fb_size,
            );
        }
        Kind::Container => {
            let mut next_ancestors = ancestors.to_vec();
            next_ancestors.push(node.id);
            for &child in &node.children {
                let Some(child_yn) = yoga_nodes[child].as_ref() else {
                    continue;
                };
                // Yoga's layout_left/top are relative to the parent's border box
                // (padding is already baked into those offsets).
                emit_node(
                    nodes,
                    child,
                    child_yn,
                    border.left,
                    border.top,
                    &next_ancestors,
                    fb_size,
                    white,
                    texture_size,
                    hits,
                    sprites,
                    yoga_nodes,
                );
            }
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
    uv: [f32; 4],
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
    s.uv = uv;
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
    use crate::style;

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
    fn column_stacks_children() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            style!(width: 100 pt, flex_direction: column),
            |ui| {
                ui.node(UiId(2), style!(height: 20 pt, width: 100 pt), |_| {});
                ui.node(UiId(3), style!(height: 30 pt, width: 100 pt), |_| {});
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
    fn flex_one_shares_row() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            style!(
                flex_direction: row,
                width: 100 pt,
                height: 40 pt,
            ),
            |ui| {
                ui.node(UiId(2), style!(flex: 1, height: 40 pt), |_| {});
                ui.node(UiId(3), style!(flex: 1, height: 40 pt), |_| {});
            },
        );
        let hits = layout_hits(&mut ui, [200.0, 200.0]);
        let a = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        let b = hits.iter().find(|(id, _)| *id == UiId(3)).unwrap().1;
        assert!((a.width - 50.0).abs() < 1e-3, "a.width={}", a.width);
        assert!((b.width - 50.0).abs() < 1e-3, "b.width={}", b.width);
        assert!((b.left - 50.0).abs() < 1e-3);
    }

    #[test]
    fn percent_width_of_parent() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            style!(width: 200 pt, height: 50 pt),
            |ui| {
                ui.node(UiId(2), style!(width: 50 %, height: 20 pt), |_| {});
            },
        );
        let hits = layout_hits(&mut ui, [400.0, 400.0]);
        let child = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        assert!((child.width - 100.0).abs() < 1e-3);
    }

    #[test]
    fn absolute_bottom_bar() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            style!(width: 200 pt, height: 300 pt),
            |ui| {
                ui.node(
                    UiId(2),
                    style!(
                        position: absolute,
                        width: 100 %,
                        bottom: 0 pt,
                        height: 40 pt,
                    ),
                    |_| {},
                );
            },
        );
        let hits = layout_hits(&mut ui, [400.0, 400.0]);
        let bar = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        assert!((bar.height - 40.0).abs() < 1e-3);
        assert!((bar.top - 260.0).abs() < 1e-3, "bar.top={}", bar.top);
        assert!((bar.width - 200.0).abs() < 1e-3);
    }

    #[test]
    fn text_width_is_glyph_count() {
        let font = Font::grid(TextureId::from_raw(1), 10, 1, '0');
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.text(UiId(1), &font, [10.0, 16.0], Style::default(), "123");
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
            style!(width: 100 pt, height: 50 pt, fit: Fit::Contain),
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
    fn image_uv_uses_cell_size_for_contain() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        // Atlas 100x50, UV covers left half → cell 50x50.
        ui.image(
            UiId(1),
            TextureId::from_raw(5),
            style!(
                width: 100 pt,
                height: 50 pt,
                fit: Fit::Contain,
                uv: [0.0, 0.0, 0.5, 1.0],
            ),
        );
        ui.layout([200.0, 200.0], TextureId::from_raw(0), |id| {
            if id == TextureId::from_raw(5) {
                (100, 50)
            } else {
                (0, 0)
            }
        });
        assert_eq!(ui.sprites.len(), 1);
        let s = &ui.sprites[0];
        assert!((s.size[0] - 50.0).abs() < 1e-3, "w={}", s.size[0]);
        assert!((s.size[1] - 50.0).abs() < 1e-3, "h={}", s.size[1]);
        assert_eq!(s.uv, [0.0, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn hit_test_topmost_and_ancestors() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            style!(width: 100 pt, height: 100 pt),
            |ui| {
                ui.node(UiId(2), style!(width: 40 pt, height: 40 pt), |_| {});
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
    fn padding_not_double_counted() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(1),
            style!(width: 100 pt, height: 100 pt, padding: 10 pt),
            |ui| {
                ui.node(UiId(2), style!(width: 20 pt, height: 20 pt), |_| {});
            },
        );
        let hits = layout_hits(&mut ui, [200.0, 200.0]);
        let child = hits.iter().find(|(id, _)| *id == UiId(2)).unwrap().1;
        assert!(
            (child.left - 10.0).abs() < 1e-3 && (child.top - 10.0).abs() < 1e-3,
            "child origin {:?}",
            (child.left, child.top)
        );
    }

    #[test]
    fn later_sibling_wins_hit() {
        let mut ui = Ui::default();
        ui.begin_frame([0.0, 0.0], false, false);
        ui.node(
            UiId(10),
            style!(
                flex_direction: row,
                width: 50 pt,
                height: 50 pt,
            ),
            |ui| {
                ui.node(UiId(11), style!(width: 50 pt, height: 50 pt), |_| {});
            },
        );
        ui.node(
            UiId(20),
            style!(
                position: absolute,
                width: 50 pt,
                height: 50 pt,
                top: 0 pt,
                left: 0 pt,
            ),
            |_| {},
        );
        ui.layout([200.0, 200.0], TextureId::from_raw(0), empty_size);
        ui.begin_frame([25.0, 25.0], true, false);
        assert!(ui.hovered(UiId(20)));
        assert!(!ui.hovered(UiId(11)));
    }
}
