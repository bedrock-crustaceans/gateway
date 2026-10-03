use crate::network::codec::trace_fields;
use crate::network::source::Source;
use crate::ui::capture::{Capture, PacketEntry};
use crate::ui::theme;
use bedrock::protocol::trace::FieldSpan;
use egui::{vec2, Align, Align2, Color32, FontId, Label, Layout, Panel, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextStyle, Ui, UiBuilder};
use egui_phosphor::regular;
use std::collections::BTreeSet;
use std::ops::Range;

const BYTES_PER_ROW: usize = 16;
const PREVIEW_BYTES: usize = 8;
const HEADER_COLOR: (u8, u8, u8) = (150, 150, 150);
const PALETTE: [(u8, u8, u8); 8] = [
    (86, 156, 214),
    (206, 145, 120),
    (181, 206, 168),
    (197, 134, 192),
    (220, 220, 170),
    (78, 201, 176),
    (244, 135, 113),
    (156, 220, 254),
];

pub struct Inspection {
    pub index: usize,
    name: &'static str,
    frame: Vec<u8>,
    debug: String,
    spans: Vec<FieldSpan>,
    colors: Vec<(u8, u8, u8)>,
    parents: Vec<Option<usize>>,
    has_children: Vec<bool>,
    expanded: BTreeSet<usize>,
    rows: Vec<usize>,
    display: Vec<Option<usize>>,
    selected: Option<usize>,
    hovered: Option<usize>,
    last_hovered: Option<usize>,
    scroll_fields_to: Option<usize>,
    scroll_hex_to: Option<usize>,
    subtitle: String,
    source: Source,
}

#[derive(Copy, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum InspectorTab {
    #[default]
    Fields,
    Hex,
    Debug,
}

impl Inspection {
    pub fn new(index: usize, entry: &PacketEntry) -> Self {
        let frame = entry.frame.clone();
        let spans = trace_fields(&frame);

        let colors = (0..spans.len()).map(|i| if i == 0 { HEADER_COLOR } else { PALETTE[(i - 1) % PALETTE.len()] }).collect();
        let parents = parents(&spans);
        let mut has_children = vec![false; spans.len()];
        for parent in parents.iter().flatten() {
            has_children[*parent] = true;
        }

        let mut inspection = Self {
            index,
            name: entry.name(),
            debug: format!("{:#?}", entry.packet),
            frame,
            spans,
            colors,
            parents,
            has_children,
            expanded: BTreeSet::new(),
            rows: Vec::new(),
            display: Vec::new(),
            selected: None,
            hovered: None,
            last_hovered: None,
            scroll_fields_to: None,
            scroll_hex_to: None,
            subtitle: format!(
                "id {}  ·  {} bytes  ·  {}  ·  {}",
                entry.packet.id(),
                entry.frame.len(),
                entry.addr,
                entry.timestamp.format("%H:%M:%S%.3f"),
            ),
            source: entry.source,
        };
        inspection.refresh();
        inspection
    }

    fn refresh(&mut self) {
        self.rows = visible_rows(&self.parents, &self.expanded);
        self.display = display_spans(&self.spans, &self.parents, &self.expanded, self.frame.len());
    }

    fn toggle(&mut self, span: usize) {
        if !self.expanded.remove(&span) {
            self.expanded.insert(span);
        }
        self.refresh();
    }

    fn expand_all(&mut self) {
        self.expanded = (0..self.spans.len()).filter(|&i| self.has_children[i]).collect();
        self.refresh();
    }

    fn collapse_all(&mut self) {
        self.expanded.clear();
        self.refresh();
    }

    fn chain(&self, span: usize) -> Vec<usize> {
        let mut chain = vec![span];
        let mut current = self.parents[span];
        while let Some(parent) = current {
            chain.push(parent);
            current = self.parents[parent];
        }
        chain.reverse();
        chain
    }

    fn path(&self, span: usize) -> String {
        let parts: Vec<String> = self.chain(span).iter().map(|&i| label(&self.spans[i])).collect();
        let s = &self.spans[span];
        format!("{}  ({}..{}, {} B)", parts.join("."), s.start, s.end, s.end - s.start)
    }
}

fn parents(spans: &[FieldSpan]) -> Vec<Option<usize>> {
    let mut stack: Vec<usize> = Vec::new();
    spans
        .iter()
        .enumerate()
        .map(|(i, span)| {
            while stack.last().is_some_and(|&top| spans[top].depth >= span.depth) {
                stack.pop();
            }
            let parent = stack.last().copied();
            stack.push(i);
            parent
        })
        .collect()
}

fn is_visible(span: usize, parents: &[Option<usize>], expanded: &BTreeSet<usize>) -> bool {
    let mut current = parents[span];
    while let Some(parent) = current {
        if !expanded.contains(&parent) {
            return false;
        }
        current = parents[parent];
    }
    true
}

fn visible_rows(parents: &[Option<usize>], expanded: &BTreeSet<usize>) -> Vec<usize> {
    (0..parents.len()).filter(|&i| is_visible(i, parents, expanded)).collect()
}

fn display_spans(spans: &[FieldSpan], parents: &[Option<usize>], expanded: &BTreeSet<usize>, len: usize) -> Vec<Option<usize>> {
    let mut display = vec![None; len];
    for (i, span) in spans.iter().enumerate() {
        if is_visible(i, parents, expanded) {
            let end = span.end.min(len);
            display[span.start.min(end)..end].fill(Some(i));
        }
    }
    display
}

fn label(span: &FieldSpan) -> String {
    match span.index {
        Some(i) => format!("[{i}]"),
        None => span.name.to_string(),
    }
}

fn tint((r, g, b): (u8, u8, u8), alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, alpha)
}

fn row_offset(ui: &Ui, row: usize) -> f32 {
    row as f32 * (ui.text_style_height(&TextStyle::Monospace) + ui.spacing().item_spacing.y)
}

pub fn inspector(ui: &mut Ui, capture: &mut Capture) {
    Panel::right("inspector")
        .resizable(true)
        .default_size(620.)
        .frame(theme::panel_frame(theme::SURFACE))
        .show(ui, |ui| {
            let tab = &mut capture.inspector_tab;
            let Some(inspection) = capture.inspection.as_mut() else {
                ui.add_space(40.);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Select a packet to inspect it").color(theme::TEXT_DIM));
                });
                return;
            };

            ui.horizontal(|ui| {
                let (dir, color) = theme::direction(inspection.source);
                theme::badge(ui, &dir, color);
                ui.label(RichText::new(inspection.name).heading().strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.small_button(regular::COPY).on_hover_text("Copy debug output").clicked() {
                        ui.ctx().copy_text(inspection.debug.clone());
                    }
                });
            });
            ui.label(RichText::new(&inspection.subtitle).small().color(theme::TEXT_DIM));
            ui.add_space(6.);

            let tabs = [
                (InspectorTab::Fields, "Fields".to_string()),
                (InspectorTab::Hex, "Hex".to_string()),
                (InspectorTab::Debug, "Debug".to_string()),
            ];
            ui.horizontal(|ui| {
                theme::tabs(ui, tab, &tabs);
                if *tab != InspectorTab::Debug && inspection.has_children.contains(&true) {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(regular::ARROWS_IN_SIMPLE).on_hover_text("Collapse all fields").clicked() {
                            inspection.collapse_all();
                        }
                        if ui.small_button(regular::ARROWS_OUT_SIMPLE).on_hover_text("Expand all fields").clicked() {
                            inspection.expand_all();
                        }
                    });
                }
            });
            ui.add_space(8.);

            inspection.last_hovered = inspection.hovered.take();

            match tab {
                InspectorTab::Fields => {
                    let half = ui.available_height() * 0.45;
                    Panel::bottom("hex")
                        .resizable(true)
                        .default_size(half)
                        .frame(egui::Frame::new().inner_margin(egui::Margin { top: 8, ..Default::default() }))
                        .show(ui, |ui| hex_view(ui, inspection));
                    field_list(ui, inspection);
                }
                InspectorTab::Hex => hex_view(ui, inspection),
                InspectorTab::Debug => {
                    ScrollArea::both().id_salt("debug").auto_shrink(false).show(ui, |ui| {
                        ui.add(Label::new(RichText::new(&inspection.debug).monospace()).extend());
                    });
                }
            }

            if inspection.hovered != inspection.last_hovered {
                ui.ctx().request_repaint();
            }
        });
}

fn field_list(ui: &mut Ui, inspection: &mut Inspection) {
    let mut scroll = ScrollArea::both().id_salt("fields").auto_shrink(false);
    if let Some(row) = inspection.scroll_fields_to.take() {
        scroll = scroll.vertical_scroll_offset(row_offset(ui, row));
    }

    let focus = inspection.last_hovered.or(inspection.selected);
    let row_height = ui.text_style_height(&TextStyle::Monospace);
    let mut toggled = None;

    scroll.show_rows(ui, row_height, inspection.rows.len(), |ui, rows| {
        for row in rows {
            let i = inspection.rows[row];
            let span = &inspection.spans[i];
            let len = inspection.frame.len();
            let bytes = &inspection.frame[span.start.min(len)..span.end.min(len)];
            let preview: Vec<String> = bytes.iter().take(PREVIEW_BYTES).map(|b| format!("{b:02X}")).collect();
            let more = if bytes.len() > PREVIEW_BYTES { " .." } else { "" };
            let color = inspection.colors[i];

            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
            let mut row = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
            {
                let ui = &mut row;
                ui.spacing_mut().item_spacing.x = 0.;
                ui.add_space(span.depth as f32 * 14.);

                let caret = match (inspection.has_children[i], inspection.expanded.contains(&i)) {
                    (false, _) => " ",
                    (true, false) => regular::CARET_RIGHT,
                    (true, true) => regular::CARET_DOWN,
                };
                let caret = ui.add(
                    Label::new(RichText::new(caret).size(row_height * 0.9).color(theme::TEXT_DIM))
                        .sense(Sense::click())
                        .selectable(false),
                );
                ui.add_space(4.);

                let text = format!(
                    "{:<width$} {:>5}..{:<5} {}{more}",
                    label(span),
                    span.start,
                    span.end,
                    preview.join(" "),
                    width = 30usize.saturating_sub(span.depth * 2),
                );
                let mut rich = RichText::new(text).monospace().color(tint(color, 255));
                if focus == Some(i) {
                    rich = rich.background_color(tint(color, 70)).strong();
                }
                let response = ui.add(Label::new(rich).sense(Sense::click()).extend().selectable(false));

                if response.hovered() || caret.hovered() {
                    inspection.hovered = Some(i);
                }
                if inspection.has_children[i] && (caret.clicked() || response.double_clicked()) {
                    toggled = Some(i);
                }
                if response.clicked() {
                    inspection.selected = Some(i);
                    inspection.scroll_hex_to = Some(span.start / BYTES_PER_ROW);
                }
            };
        }
    });

    if let Some(i) = toggled {
        inspection.toggle(i);
    }
}

fn hex_view(ui: &mut Ui, inspection: &mut Inspection) {
    let mut scroll = ScrollArea::both().id_salt("hex").auto_shrink(false);
    if let Some(row) = inspection.scroll_hex_to.take() {
        scroll = scroll.vertical_scroll_offset(row_offset(ui, row));
    }

    let font = TextStyle::Monospace.resolve(ui.style());
    let char_w = ui.ctx().fonts_mut(|f| f.glyph_width(&font, '0'));
    let layout = HexLayout {
        char_w,
        hex_x: char_w * 8.,
        ascii_x: char_w * (8. + 3. * BYTES_PER_ROW as f32 + 1.),
    };
    let row_height = ui.text_style_height(&TextStyle::Monospace);
    let width = layout.ascii_x + char_w * BYTES_PER_ROW as f32;
    let rows = inspection.frame.len().div_ceil(BYTES_PER_ROW);
    let focus = inspection.last_hovered.or(inspection.selected);

    scroll.show_rows(ui, row_height, rows, |ui, rows| {
        for row in rows {
            let (rect, response) = ui.allocate_exact_size(vec2(width, row_height), Sense::click());
            let start = row * BYTES_PER_ROW;
            let end = (start + BYTES_PER_ROW).min(inspection.frame.len());

            paint_hex_row(ui, inspection, &layout, rect, start..end, focus, &font);

            let hit = |pos: Pos2| layout.hit(pos.x - rect.left()).map(|k| start + k).filter(|&o| o < end);

            if let Some(offset) = response.hover_pos().and_then(hit) {
                inspection.hovered = inspection.display[offset];
                if let Some(span) = inspection.display[offset] {
                    let path = inspection.path(span);
                    response.clone().on_hover_ui_at_pointer(|ui| {
                        ui.label(path);
                    });
                }
            }
            if response.clicked()
                && let Some(span) = response.interact_pointer_pos().and_then(hit).and_then(|o| inspection.display[o])
            {
                inspection.selected = Some(span);
                inspection.scroll_fields_to = inspection.rows.iter().position(|&r| r == span);
            }
        }
    });
}

struct HexLayout {
    char_w: f32,
    hex_x: f32,
    ascii_x: f32,
}

impl HexLayout {
    fn hex_cell(&self, k: usize) -> f32 {
        self.hex_x + 3. * self.char_w * k as f32
    }

    fn ascii_cell(&self, k: usize) -> f32 {
        self.ascii_x + self.char_w * k as f32
    }

    fn hit(&self, x: f32) -> Option<usize> {
        let k = if x >= self.ascii_x {
            (x - self.ascii_x) / self.char_w
        } else {
            (x - self.hex_x + self.char_w * 0.5) / (3. * self.char_w)
        };
        (k >= 0.).then_some(k as usize).filter(|&k| k < BYTES_PER_ROW)
    }
}

fn paint_hex_row(ui: &Ui, inspection: &Inspection, layout: &HexLayout, rect: Rect, bytes: Range<usize>, focus: Option<usize>, font: &FontId) {
    let painter = ui.painter();
    let x = |dx: f32| rect.left() + dx;
    let half = layout.char_w * 0.5;

    let chains: Vec<Vec<usize>> = bytes.clone().map(|o| inspection.display[o].map(|s| inspection.chain(s)).unwrap_or_default()).collect();
    let levels = chains.iter().map(Vec::len).max().unwrap_or(0);

    for level in 0..levels {
        let inset = level as f32 * 2.;
        let mut run_start = 0;
        while run_start < chains.len() {
            let key = chains[run_start].get(level).copied();
            let mut run_end = run_start + 1;
            while run_end < chains.len() && chains[run_end].get(level).copied() == key {
                run_end += 1;
            }

            if let Some(span) = key {
                let color = inspection.colors[span];
                let is_parent = level + 1 < chains[run_start].len();
                let fill = match (focus == Some(span), is_parent) {
                    (true, _) => tint(color, 90),
                    (false, true) => tint(color, 14),
                    (false, false) => tint(color, 30),
                };
                let y = rect.y_range().expand(1. - inset * 0.5);
                let hex = Rect::from_x_y_ranges(x(layout.hex_cell(run_start) - half + 1. + inset)..=x(layout.hex_cell(run_end) - half - 1. - inset), y);

                painter.rect_filled(hex, 4., fill);
                if is_parent {
                    painter.rect_stroke(hex, 4., Stroke::new(1., tint(color, 90)), StrokeKind::Inside);
                } else {
                    let ascii = Rect::from_x_y_ranges(x(layout.ascii_cell(run_start) + 0.5)..=x(layout.ascii_cell(run_end) - 0.5), y);
                    painter.rect_filled(ascii, 2., fill);
                }
            }

            run_start = run_end;
        }
    }

    let y = rect.center().y;
    painter.text(Pos2::new(x(0.), y), Align2::LEFT_CENTER, format!("{:06X}", bytes.start), font.clone(), theme::TEXT_DIM);

    for (k, chain) in chains.iter().enumerate() {
        let byte = inspection.frame[bytes.start + k];
        let color = match chain.last() {
            Some(_) if focus.is_some_and(|f| chain.contains(&f)) => theme::TEXT,
            Some(&span) => tint(inspection.colors[span], 255),
            None => theme::TEXT_DIM,
        };
        let ch = if byte.is_ascii_graphic() || byte == b' ' { byte as char } else { '.' };

        painter.text(Pos2::new(x(layout.hex_cell(k)), y), Align2::LEFT_CENTER, format!("{byte:02X}"), font.clone(), color);
        painter.text(Pos2::new(x(layout.ascii_cell(k)), y), Align2::LEFT_CENTER, ch, font.clone(), color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(name: &'static str, depth: usize, start: usize, end: usize) -> FieldSpan {
        FieldSpan { name, index: None, depth, start, end }
    }

    fn sample() -> Vec<FieldSpan> {
        vec![
            span("header", 0, 0, 1),
            span("outer", 0, 1, 5),
            span("a", 1, 1, 2),
            span("inner", 1, 2, 5),
            span("b", 2, 2, 3),
            span("c", 2, 3, 5),
            span("tail", 0, 5, 6),
        ]
    }

    #[test]
    fn parents_follow_depth() {
        assert_eq!(parents(&sample()), vec![None, None, Some(1), Some(1), Some(3), Some(3), None]);
    }

    #[test]
    fn collapsed_fields_hide_children_and_own_their_bytes() {
        let spans = sample();
        let parents = parents(&spans);
        let collapsed = BTreeSet::new();

        assert_eq!(visible_rows(&parents, &collapsed), vec![0, 1, 6]);
        assert_eq!(display_spans(&spans, &parents, &collapsed, 6), vec![Some(0), Some(1), Some(1), Some(1), Some(1), Some(6)]);
    }

    #[test]
    fn expanding_reveals_one_level_at_a_time() {
        let spans = sample();
        let parents = parents(&spans);
        let mut expanded = BTreeSet::from([1]);

        assert_eq!(visible_rows(&parents, &expanded), vec![0, 1, 2, 3, 6]);
        assert_eq!(display_spans(&spans, &parents, &expanded, 6)[1..5], [Some(2), Some(3), Some(3), Some(3)]);

        expanded.insert(3);
        assert_eq!(display_spans(&spans, &parents, &expanded, 6)[1..5], [Some(2), Some(4), Some(5), Some(5)]);

        expanded.remove(&1);
        assert_eq!(visible_rows(&parents, &expanded), vec![0, 1, 6]);
    }
}
