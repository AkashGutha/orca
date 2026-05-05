mod app;
mod controls;
mod panes;
mod run_controller;
mod settings_panel;
mod ui_config;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("🐋 ORCA")
            .with_icon(orca_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "🐋 ORCA",
        options,
        Box::new(|_cc| Ok(Box::new(app::OrcaApp::default()))),
    )
}

fn orca_icon() -> eframe::egui::IconData {
    const SIZE: u32 = 64;
    let mut rgba = vec![0; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - 32.0;
            let dy = y as f32 - 32.0;
            let background = dx * dx + dy * dy <= 31.0 * 31.0;
            let body = ellipse(x, y, 33.0, 35.0, 22.0, 13.0);
            let belly = ellipse(x, y, 38.0, 39.0, 14.0, 7.0);
            let head_patch = ellipse(x, y, 47.0, 31.0, 8.0, 5.0);
            let dorsal =
                point_in_triangle(x as f32, y as f32, (27.0, 23.0), (34.0, 7.0), (38.0, 27.0));
            let tail_top =
                point_in_triangle(x as f32, y as f32, (13.0, 33.0), (3.0, 22.0), (17.0, 29.0));
            let tail_bottom =
                point_in_triangle(x as f32, y as f32, (13.0, 36.0), (4.0, 47.0), (18.0, 40.0));
            let eye = ellipse(x, y, 49.0, 29.0, 2.0, 2.0);
            let highlight = ellipse(x, y, 51.0, 27.0, 1.0, 1.0);

            let color = if eye {
                [12, 20, 28, 255]
            } else if highlight {
                [255, 255, 255, 255]
            } else if belly || head_patch {
                [246, 248, 250, 255]
            } else if body || dorsal || tail_top || tail_bottom {
                [12, 20, 28, 255]
            } else if background {
                [111, 211, 245, 255]
            } else {
                [0, 0, 0, 0]
            };

            let offset = ((y * SIZE + x) * 4) as usize;
            rgba[offset..offset + 4].copy_from_slice(&color);
        }
    }

    eframe::egui::IconData {
        rgba,
        width: SIZE,
        height: SIZE,
    }
}

fn ellipse(x: u32, y: u32, cx: f32, cy: f32, rx: f32, ry: f32) -> bool {
    let dx = (x as f32 - cx) / rx;
    let dy = (y as f32 - cy) / ry;
    dx * dx + dy * dy <= 1.0
}

fn point_in_triangle(p: f32, q: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
    let area = |p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)| {
        (p1.0 * (p2.1 - p3.1) + p2.0 * (p3.1 - p1.1) + p3.0 * (p1.1 - p2.1)).abs()
    };
    let point = (p, q);
    let total = area(a, b, c);
    let parts = area(point, b, c) + area(a, point, c) + area(a, b, point);
    (total - parts).abs() < 0.5
}
