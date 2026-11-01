use eframe::egui::{self, Color32, FontId, RichText, TextStyle};
pub(super) const CANVAS: Color32 = Color32::from_rgb(246, 247, 243);
pub(super) const PANEL: Color32 = Color32::from_rgb(255, 255, 253);
pub(super) const BORDER: Color32 = Color32::from_rgb(222, 226, 218);
pub(super) const ACCENT: Color32 = Color32::from_rgb(207, 88, 46);
pub(super) const MUTED: Color32 = Color32::from_rgb(113, 121, 116);
pub(super) const GREEN: Color32 = Color32::from_rgb(37, 128, 95);
pub(super) const AMBER: Color32 = Color32::from_rgb(154, 108, 36);

pub(super) fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windows = std::env::var_os("WINDIR").map(std::path::PathBuf::from);
    if let Some(root) = windows {
        for (name, file) in [("interface", "segoeui.ttf"), ("display", "seguisb.ttf")] {
            if let Ok(bytes) = std::fs::read(root.join("Fonts").join(file)) {
                fonts
                    .font_data
                    .insert(name.into(), egui::FontData::from_owned(bytes).into());
            }
        }
    }
    let proportional = fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default();
    if fonts.font_data.contains_key("interface") {
        proportional.insert(0, "interface".into());
    }
    let mut display = proportional.clone();
    if fonts.font_data.contains_key("display") {
        display.insert(0, "display".into());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("display".into()), display);
    ctx.set_fonts(fonts);
}

pub(super) fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(Color32::from_rgb(31, 41, 36));
    style.visuals.widgets.noninteractive.fg_stroke =
        egui::Stroke::new(1.0_f32, Color32::from_rgb(31, 41, 36));
    style.visuals.window_stroke = egui::Stroke::new(1.0_f32, BORDER);
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = PANEL;
    style.visuals.extreme_bg_color = Color32::from_rgb(255, 255, 253);
    style.visuals.faint_bg_color = PANEL;
    style.visuals.selection.bg_fill = Color32::from_rgb(251, 226, 211);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(239, 242, 235);
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, BORDER);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(229, 234, 224);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(219, 227, 214);
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 10.0);
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(32.0, egui::FontFamily::Name("display".into())),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(15.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    ctx.set_style(style);
}
pub(super) fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(PANEL)
        .stroke(egui::Stroke::new(1.0_f32, BORDER))
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(24)
}
pub(super) fn title(ui: &mut egui::Ui, eyebrow: &str, heading: &str, detail: &str) {
    ui.label(RichText::new(eyebrow).small().color(ACCENT).strong());
    ui.heading(heading);
    ui.label(RichText::new(detail).color(MUTED));
    ui.add_space(12.0);
}
pub(super) fn primary(label: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
        .fill(Color32::from_rgb(33, 49, 40))
}
