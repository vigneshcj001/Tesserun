fn main() -> eframe::Result {
    let mut rgba = vec![0_u8; 64 * 64 * 4];
    for y in 0..64_usize {
        for x in 0..64_usize {
            let corner_x = x.clamp(12, 51);
            let corner_y = y.clamp(12, 51);
            if x.abs_diff(corner_x).pow(2) + y.abs_diff(corner_y).pow(2) <= 121 {
                let letter = ((18..46).contains(&x) && (16..24).contains(&y))
                    || ((28..36).contains(&x) && (24..49).contains(&y));
                let color = if letter {
                    [255, 255, 250, 255]
                } else {
                    [207, 88, 46, 255]
                };
                let index = (y * 64 + x) * 4;
                rgba[index..index + 4].copy_from_slice(&color);
            }
        }
    }
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_icon(eframe::egui::IconData {
                rgba,
                width: 64,
                height: 64,
            })
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([900.0, 650.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Tesserun",
        options,
        Box::new(|cc| Ok(Box::new(tesserun::ui::Workspace::new(cc)?))),
    )
}
