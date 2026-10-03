use crate::network::source::Source;
use egui::{Color32, Context, CornerRadius, FontFamily, FontId, Margin, Response, RichText, Sense, Stroke, TextStyle, ThemePreference, Ui, Visuals, vec2};

pub const BG: Color32 = Color32::from_rgb(0x14, 0x16, 0x1b);
pub const SURFACE: Color32 = Color32::from_rgb(0x1b, 0x1e, 0x25);
pub const SURFACE_RAISED: Color32 = Color32::from_rgb(0x23, 0x27, 0x30);
pub const BORDER: Color32 = Color32::from_rgb(0x2c, 0x31, 0x3b);
pub const TEXT: Color32 = Color32::from_rgb(0xd5, 0xd9, 0xe0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x86, 0x8d, 0x99);
pub const ACCENT: Color32 = Color32::from_rgb(0x5b, 0x9c, 0xf5);
pub const CLIENT: Color32 = Color32::from_rgb(0x7c, 0xcf, 0x8a);
pub const SERVER: Color32 = Color32::from_rgb(0x6a, 0xa8, 0xf7);
pub const DANGER: Color32 = Color32::from_rgb(0xe0, 0x7a, 0x7a);

pub fn apply(ctx: &Context) {
    ctx.set_theme(ThemePreference::Dark);

    let mut visuals = Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = SURFACE;
    visuals.extreme_bg_color = Color32::from_rgb(0x0f, 0x11, 0x15);
    visuals.faint_bg_color = SURFACE;
    visuals.code_bg_color = SURFACE_RAISED;
    visuals.window_stroke = Stroke::new(1., BORDER);
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.selection.stroke = Stroke::new(1., ACCENT);
    visuals.hyperlink_color = ACCENT;
    visuals.striped = true;

    let radius = CornerRadius::same(5);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
    }
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1., BORDER);
    visuals.widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2c, 0x31, 0x3c);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1., BORDER);

    ctx.set_visuals(visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = vec2(8., 6.);
        style.spacing.button_padding = vec2(8., 4.);
        style.spacing.interact_size.y = 24.;
        style.text_styles.insert(TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace));
        style.text_styles.insert(TextStyle::Body, FontId::new(13.5, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Small, FontId::new(11.5, FontFamily::Proportional));
        style.text_styles.insert(TextStyle::Heading, FontId::new(17., FontFamily::Proportional));
    });
}

pub fn panel_frame(fill: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(10, 8))
}

pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, items: &[(T, String)]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.;
        for (value, label) in items {
            let active = *current == *value;
            let text = RichText::new(label).color(if active { TEXT } else { TEXT_DIM });
            let response = ui.add(egui::Label::new(text).sense(Sense::click()).selectable(false));
            let rect = response.rect.expand2(vec2(8., 5.));
            if active {
                ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(2., ACCENT));
            } else if response.hovered() {
                ui.painter().hline(rect.x_range(), rect.bottom(), Stroke::new(2., BORDER));
            }
            if response.clicked() {
                *current = *value;
            }
            ui.add_space(14.);
        }
    });
}

pub fn direction(source: Source) -> (String, Color32) {
    let arrow = egui_phosphor::regular::ARROW_RIGHT;
    match source {
        Source::Client => (format!("C {arrow} S"), CLIENT),
        Source::Server => (format!("S {arrow} C"), SERVER),
    }
}

pub fn badge(ui: &mut Ui, text: &str, color: Color32) -> Response {
    egui::Frame::new()
        .fill(color.gamma_multiply(0.15))
        .corner_radius(3)
        .inner_margin(Margin::symmetric(5, 1))
        .show(ui, |ui| ui.add(egui::Label::new(RichText::new(text).small().strong().color(color)).selectable(false)))
        .inner
}
