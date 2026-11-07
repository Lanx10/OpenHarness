use eframe::egui;
use std::env;

mod openharness_view;
use openharness_view::OpenHarnessView;
use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut screenshot_path = None;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--screenshot" {
            screenshot_path = args.next().map(PathBuf::from);
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 900.0])
            .with_decorations(false)
            .with_title("OpenHarness"),
        ..Default::default()
    };

    eframe::run_native(
        "OpenHarness",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            setup_brutalist_style(&cc.egui_ctx);
            if let Ok(data) = std::fs::read("assets/SpaceGrotesk.ttf") {
                let mut fonts = egui::FontDefinitions::default();
                fonts
                    .font_data
                    .insert("spacegrotesk".to_owned(), egui::FontData::from_owned(data));
                for family in [
                    &egui::FontFamily::Proportional,
                    &egui::FontFamily::Monospace,
                ] {
                    fonts
                        .families
                        .get_mut(family)
                        .expect("font family exists")
                        .insert(0, "spacegrotesk".to_owned());
                }
                cc.egui_ctx.set_fonts(fonts);
            }
            Ok(Box::new(OpenHarnessApp::new(screenshot_path)))
        }),
    )
}

fn setup_brutalist_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(egui::Color32::from_rgb(220, 220, 220));
    v.panel_fill = egui::Color32::from_rgb(22, 23, 22);
    v.window_fill = egui::Color32::from_rgb(22, 23, 22);
    v.faint_bg_color = egui::Color32::from_rgb(32, 34, 32);
    v.extreme_bg_color = egui::Color32::from_rgb(27, 29, 27);
    v.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(22, 23, 22);
    v.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(46, 48, 46));
    v.widgets.inactive.rounding = egui::Rounding::same(8.0);
    v.widgets.inactive.bg_fill = egui::Color32::from_rgb(32, 34, 32);
    v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(48, 52, 48));
    v.widgets.hovered.rounding = egui::Rounding::same(8.0);
    v.widgets.hovered.bg_fill = egui::Color32::from_rgb(44, 47, 44);
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(70, 75, 70));
    v.widgets.active.rounding = egui::Rounding::same(8.0);
    v.widgets.active.bg_fill = egui::Color32::from_rgb(188, 226, 158);
    v.widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(188, 226, 158));
    ctx.set_visuals(v);
    let mut style = (*ctx.style()).clone();
    style.override_font_id = Some(egui::FontId::proportional(13.5));
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    ctx.set_style(style);
}

struct OpenHarnessApp {
    harness_view_state: OpenHarnessView,
    screenshot_path: Option<PathBuf>,
    frame_count: usize,
}

impl OpenHarnessApp {
    fn new(screenshot_path: Option<PathBuf>) -> Self {
        Self {
            harness_view_state: OpenHarnessView::default(),
            screenshot_path,
            frame_count: 0,
        }
    }
}

impl eframe::App for OpenHarnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.screenshot_path.is_some() {
            self.frame_count += 1;
            if self.frame_count == 120 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }
        }

        for event in ctx.input(|i| i.raw.events.clone()) {
            if let egui::Event::Screenshot { image, .. } = event {
                if let Some(ref path) = self.screenshot_path {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let w = image.width() as u32;
                    let h = image.height() as u32;
                    let pixels: Vec<u8> = image
                        .pixels
                        .iter()
                        .flat_map(|p| [p.r(), p.g(), p.b(), p.a()])
                        .collect();
                    if let Some(buf) = image::RgbaImage::from_raw(w, h, pixels) {
                        let _ = buf.save(path);
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        openharness_view::render(ctx, &mut self.harness_view_state);

        if self.screenshot_path.is_some() {
            ctx.request_repaint();
        }
    }
}
