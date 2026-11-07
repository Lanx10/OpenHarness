use eframe::egui::{self, Color32, Margin, Pos2, Rect, Rounding, Stroke};

pub struct OpenHarnessView {
    pub prompt: String,
    pub selected_project: Option<String>,
    pub is_code_mode: bool,
    icons: std::collections::HashMap<&'static str, egui::TextureHandle>,
    has_load_attempted: bool,
}

impl OpenHarnessView {
    fn ensure_icons_loaded(&mut self, ctx: &egui::Context) {
        if !self.has_load_attempted {
            self.icons = load_icons(ctx);
            self.has_load_attempted = true;
        }
    }
}

fn load_icons(ctx: &egui::Context) -> std::collections::HashMap<&'static str, egui::TextureHandle> {
    [
        ("home.svg", include_bytes!("../appicon/home.svg").as_slice()),
        (
            "panels.svg",
            include_bytes!("../appicon/panels.svg").as_slice(),
        ),
        ("grid.svg", include_bytes!("../appicon/grid.svg").as_slice()),
        (
            "settings.svg",
            include_bytes!("../appicon/settings.svg").as_slice(),
        ),
        ("gift.svg", include_bytes!("../appicon/gift.svg").as_slice()),
        (
            "search.svg",
            include_bytes!("../appicon/search.svg").as_slice(),
        ),
        ("edit.svg", include_bytes!("../appicon/edit.svg").as_slice()),
        (
            "folder.svg",
            include_bytes!("../appicon/folder.svg").as_slice(),
        ),
        (
            "clock.svg",
            include_bytes!("../appicon/clock.svg").as_slice(),
        ),
        (
            "wallet.svg",
            include_bytes!("../appicon/wallet.svg").as_slice(),
        ),
        (
            "rocket.svg",
            include_bytes!("../appicon/rocket.svg").as_slice(),
        ),
        ("down.svg", include_bytes!("../appicon/down.svg").as_slice()),
        ("left.svg", include_bytes!("../appicon/left.svg").as_slice()),
        (
            "right.svg",
            include_bytes!("../appicon/right.svg").as_slice(),
        ),
        (
            "close.svg",
            include_bytes!("../appicon/close.svg").as_slice(),
        ),
        (
            "minus.svg",
            include_bytes!("../appicon/minus.svg").as_slice(),
        ),
        (
            "square.svg",
            include_bytes!("../appicon/square.svg").as_slice(),
        ),
        ("back.svg", include_bytes!("../appicon/back.svg").as_slice()),
        (
            "forward.svg",
            include_bytes!("../appicon/forward.svg").as_slice(),
        ),
        (
            "local.svg",
            include_bytes!("../appicon/local.svg").as_slice(),
        ),
        ("plus.svg", include_bytes!("../appicon/plus.svg").as_slice()),
        (
            "message.svg",
            include_bytes!("../appicon/message.svg").as_slice(),
        ),
        (
            "logo.svg",
            include_bytes!("../appicon/openharness-titlebar-64-v2.svg").as_slice(),
        ),
    ]
    .into_iter()
    .filter_map(|(name, svg)| {
        let size = egui_extras::image::load_svg_bytes(svg).ok()?;
        Some((
            name,
            ctx.load_texture(name, size, egui::TextureOptions::LINEAR),
        ))
    })
    .collect()
}

fn icon_ui(
    ui: &mut egui::Ui,
    icons: &std::collections::HashMap<&'static str, egui::TextureHandle>,
    name: &str,
    size: f32,
    tint: Color32,
) {
    if let Some(texture) = icons.get(name) {
        ui.add(
            egui::Image::new(texture)
                .fit_to_exact_size(egui::vec2(size, size))
                .tint(tint),
        );
    }
}

fn icon_btn(
    ui: &mut egui::Ui,
    icons: &std::collections::HashMap<&'static str, egui::TextureHandle>,
    name: &str,
    size: f32,
    tint: Color32,
) -> egui::Response {
    if let Some(texture) = icons.get(name) {
        ui.add(
            egui::Button::image(
                egui::Image::new(texture)
                    .fit_to_exact_size(egui::vec2(size, size))
                    .tint(tint),
            )
            .fill(Color32::TRANSPARENT)
            .frame(false),
        )
    } else {
        ui.allocate_response(egui::vec2(size, size), egui::Sense::click())
    }
}

impl Default for OpenHarnessView {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            selected_project: Some("Victory Liner".into()),
            is_code_mode: true,
            icons: std::collections::HashMap::new(),
            has_load_attempted: false,
        }
    }
}

fn draw_robot(p: &egui::Painter, rect: Rect, sage: Color32, dark: Color32) {
    let x = rect.center().x;
    let y = rect.min.y;
    p.rect_filled(
        Rect::from_center_size(Pos2::new(x, y + 30.0), egui::vec2(58.0, 52.0)),
        Rounding::same(16.0),
        sage,
    );
    p.rect_filled(
        Rect::from_center_size(Pos2::new(x, y + 16.0), egui::vec2(58.0, 10.0)),
        Rounding {
            nw: 16.0,
            ne: 16.0,
            sw: 2.0,
            se: 2.0,
        },
        Color32::from_rgb(240, 248, 238),
    );
    p.circle_filled(Pos2::new(x - 11.0, y + 33.0), 3.2, dark);
    p.circle_filled(Pos2::new(x + 11.0, y + 33.0), 3.2, dark);
    p.hline((x - 4.0)..=(x + 4.0), y + 43.0, Stroke::new(1.4_f32, dark));
    let bar = y + 64.0;
    p.hline(
        (x - 38.0)..=(x + 38.0),
        bar,
        Stroke::new(3.0_f32, Color32::from_rgb(200, 205, 200)),
    );
    for (dx, w, h, col) in [
        (-36.0, 5.0, 24.0, 60),
        (-30.0, 4.0, 18.0, 80),
        (36.0, 5.0, 24.0, 60),
        (30.0, 4.0, 18.0, 80),
    ] {
        p.rect_filled(
            Rect::from_center_size(Pos2::new(x + dx, bar), egui::vec2(w, h)),
            Rounding::same(2.0),
            Color32::from_gray(col),
        );
    }
    p.circle_filled(Pos2::new(x - 14.0, bar), 4.5, sage);
    p.circle_filled(Pos2::new(x + 14.0, bar), 4.5, sage);
}

fn draw_wave(p: &egui::Painter, r: Rect, sage: Color32, dark: Color32) {
    p.rect_filled(r, Rounding::same(12.0), sage);
    for (dx, h) in [(-4.5, 7.0), (-1.5, 12.0), (1.5, 15.0), (4.5, 9.0)] {
        p.vline(
            r.center().x + dx,
            (r.center().y - h / 2.0)..=(r.center().y + h / 2.0),
            Stroke::new(1.5_f32, dark),
        );
    }
}

fn mock_landing(p: &egui::Painter, r: Rect) {
    p.rect_filled(
        Rect::from_min_max(
            Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
            Pos2::new(r.max.x - 8.0, r.max.y - 12.0),
        ),
        Rounding::same(4.0),
        Color32::from_rgb(46, 32, 24),
    );
    p.hline(
        (r.min.x + 14.0)..=(r.max.x - 14.0),
        r.min.y + 22.0,
        Stroke::new(2.0_f32, Color32::from_rgb(240, 230, 220)),
    );
    p.hline(
        (r.min.x + 20.0)..=(r.max.x - 20.0),
        r.min.y + 28.0,
        Stroke::new(1.5_f32, Color32::from_rgb(180, 160, 150)),
    );
    p.rect_filled(
        Rect::from_center_size(
            Pos2::new(r.center().x, r.min.y + 38.0),
            egui::vec2(28.0, 7.0),
        ),
        Rounding::same(2.0),
        Color32::from_rgb(220, 200, 190),
    );
}

fn mock_store(p: &egui::Painter, r: Rect) {
    let inner = Rect::from_min_max(
        Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
        Pos2::new(r.max.x - 8.0, r.max.y - 12.0),
    );
    p.rect_filled(inner, Rounding::same(4.0), Color32::from_rgb(238, 235, 228));
    p.rect_filled(
        Rect::from_min_size(
            Pos2::new(r.min.x + 12.0, r.min.y + 14.0),
            egui::vec2(28.0, 24.0),
        ),
        Rounding::same(2.0),
        Color32::from_rgb(205, 195, 185),
    );
    p.hline(
        (r.min.x + 44.0)..=(r.max.x - 14.0),
        r.min.y + 18.0,
        Stroke::new(1.5_f32, Color32::from_rgb(60, 55, 50)),
    );
    p.hline(
        (r.min.x + 44.0)..=(r.max.x - 22.0),
        r.min.y + 24.0,
        Stroke::new(1.2_f32, Color32::from_rgb(140, 130, 120)),
    );
    p.rect_filled(
        Rect::from_min_size(
            Pos2::new(r.min.x + 44.0, r.min.y + 30.0),
            egui::vec2(16.0, 5.0),
        ),
        Rounding::same(1.0),
        Color32::from_rgb(188, 226, 158),
    );
}

fn mock_portfolio(p: &egui::Painter, r: Rect) {
    let inner = Rect::from_min_max(
        Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
        Pos2::new(r.max.x - 8.0, r.max.y - 12.0),
    );
    p.rect_filled(inner, Rounding::same(4.0), Color32::from_rgb(240, 225, 220));
    p.circle_filled(
        Pos2::new(r.center().x + 14.0, r.center().y - 2.0),
        16.0,
        Color32::from_rgb(180, 205, 245),
    );
    p.circle_filled(
        Pos2::new(r.center().x - 10.0, r.center().y + 6.0),
        14.0,
        Color32::from_rgb(250, 190, 160),
    );
    p.text(
        Pos2::new(r.min.x + 14.0, r.min.y + 20.0),
        egui::Align2::LEFT_TOP,
        "Your\nName",
        egui::FontId::proportional(8.5),
        Color32::from_rgb(40, 30, 35),
    );
}

fn mock_study(p: &egui::Painter, r: Rect) {
    let inner = Rect::from_min_max(
        Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
        Pos2::new(r.max.x - 8.0, r.max.y - 12.0),
    );
    p.rect_filled(inner, Rounding::same(4.0), Color32::from_rgb(248, 250, 252));
    p.rect_filled(
        Rect::from_min_size(
            Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
            egui::vec2(inner.width(), 6.0),
        ),
        Rounding::same(1.0),
        Color32::from_rgb(99, 102, 241),
    );
    for y in [22.0, 28.0, 34.0] {
        p.hline(
            (r.min.x + 12.0)..=(r.max.x - 12.0),
            r.min.y + y,
            Stroke::new(1.0_f32, Color32::from_rgb(226, 232, 240)),
        );
        p.circle_filled(
            Pos2::new(r.min.x + 16.0, r.min.y + y),
            1.5,
            Color32::from_rgb(129, 140, 248),
        );
    }
}

fn mock_chat(p: &egui::Painter, r: Rect) {
    let inner = Rect::from_min_max(
        Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
        Pos2::new(r.max.x - 8.0, r.max.y - 12.0),
    );
    p.rect_filled(inner, Rounding::same(4.0), Color32::from_rgb(244, 244, 245));
    p.rect_filled(
        Rect::from_min_size(
            Pos2::new(r.min.x + 12.0, r.min.y + 16.0),
            egui::vec2(44.0, 8.0),
        ),
        Rounding::same(3.0),
        Color32::from_rgb(228, 228, 231),
    );
    p.rect_filled(
        Rect::from_min_size(
            Pos2::new(r.max.x - 52.0, r.min.y + 27.0),
            egui::vec2(40.0, 10.0),
        ),
        Rounding::same(3.0),
        Color32::from_rgb(188, 226, 158),
    );
}

fn mock_crm(p: &egui::Painter, r: Rect) {
    let inner = Rect::from_min_max(
        Pos2::new(r.min.x + 8.0, r.min.y + 10.0),
        Pos2::new(r.max.x - 8.0, r.max.y - 12.0),
    );
    p.rect_filled(inner, Rounding::same(4.0), Color32::from_rgb(250, 250, 250));
    p.hline(
        (r.min.x + 12.0)..=(r.min.x + 36.0),
        r.min.y + 16.0,
        Stroke::new(2.0_f32, Color32::from_rgb(30, 41, 59)),
    );
    let points = [
        Pos2::new(r.min.x + 12.0, r.min.y + 36.0),
        Pos2::new(r.min.x + 28.0, r.min.y + 32.0),
        Pos2::new(r.min.x + 44.0, r.min.y + 35.0),
        Pos2::new(r.min.x + 60.0, r.min.y + 26.0),
        Pos2::new(r.max.x - 14.0, r.min.y + 22.0),
    ];
    for pair in points.windows(2) {
        p.line_segment(
            [pair[0], pair[1]],
            Stroke::new(1.6_f32, Color32::from_rgb(59, 130, 246)),
        );
    }
}

pub fn render(ctx: &egui::Context, state: &mut OpenHarnessView) {
    state.ensure_icons_loaded(ctx);
    let bg = Color32::from_rgb(22, 23, 22);
    let rail_bg = Color32::from_rgb(18, 19, 18);
    let border = Color32::from_rgb(38, 40, 38);
    let sage = Color32::from_rgb(188, 226, 158);
    let sage_dark = Color32::from_rgb(22, 36, 19);
    let white = Color32::from_rgb(245, 245, 245);
    let muted = Color32::from_rgb(154, 160, 154);
    let dim = Color32::from_rgb(107, 112, 107);
    let panel_bg = Color32::from_rgb(29, 31, 29);
    let input_bg = Color32::from_rgb(27, 29, 27);
    let icons = &state.icons;

    egui::TopBottomPanel::top("openharness_titlebar")
        .exact_height(39.0)
        .frame(
            egui::Frame::none()
                .fill(bg)
                .stroke(Stroke::new(1.0_f32, border)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.add_space(12.0);
                icon_btn(ui, icons, "back.svg", 18.0, muted);
                ui.add_space(6.0);
                icon_btn(ui, icons, "forward.svg", 18.0, dim);
                ui.add_space(16.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(34, 36, 34))
                    .rounding(Rounding::same(14.0))
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(45, 48, 45)))
                    .inner_margin(Margin::symmetric(2.0, 2.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (label, selected) in
                                [("Code", state.is_code_mode), ("Chat", !state.is_code_mode)]
                            {
                                let fill = if selected {
                                    Color32::from_rgb(245, 245, 245)
                                } else {
                                    Color32::TRANSPARENT
                                };
                                let color = if selected {
                                    Color32::from_rgb(18, 18, 18)
                                } else {
                                    muted
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(label)
                                                .color(color)
                                                .size(12.5)
                                                .strong(),
                                        )
                                        .fill(fill)
                                        .rounding(Rounding::same(12.0))
                                        .min_size(egui::vec2(48.0, 22.0)),
                                    )
                                    .clicked()
                                {
                                    state.is_code_mode = label == "Code";
                                }
                            }
                        });
                    });
                ui.add_space(18.0);
                egui::Frame::none()
                    .fill(Color32::from_rgb(32, 34, 32))
                    .rounding(Rounding {
                        nw: 6.0,
                        ne: 6.0,
                        sw: 0.0,
                        se: 0.0,
                    })
                    .stroke(Stroke::new(1.0_f32, border))
                    .inner_margin(Margin::symmetric(14.0, 6.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("New chat").size(12.5).color(white));
                            ui.add_space(10.0);
                            icon_btn(ui, icons, "close.svg", 14.0, dim);
                        });
                    });
                ui.add_space(8.0);
                icon_btn(ui, icons, "plus.svg", 16.0, dim);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(12.0);
                    icon_btn(ui, icons, "close.svg", 14.0, muted);
                    icon_btn(ui, icons, "square.svg", 14.0, muted);
                    icon_btn(ui, icons, "minus.svg", 14.0, muted);
                    icon_btn(ui, icons, "panels.svg", 16.0, muted);
                    icon_btn(ui, icons, "folder.svg", 16.0, muted);
                    icon_btn(ui, icons, "code.svg", 16.0, muted);
                });
            });
        });

    egui::SidePanel::left("openharness_rail")
        .exact_width(46.0)
        .frame(
            egui::Frame::none()
                .fill(rail_bg)
                .inner_margin(Margin::symmetric(7.0, 10.0))
                .stroke(Stroke::new(1.0_f32, border)),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
                let house_bg = if response.hovered() {
                    Color32::from_rgb(46, 52, 46)
                } else {
                    Color32::from_rgb(38, 42, 38)
                };
                ui.painter()
                    .rect_filled(rect, Rounding::same(8.0), house_bg);
                ui.allocate_ui_at_rect(rect, |ui| {
                    ui.centered_and_justified(|ui| {
                        icon_ui(ui, icons, "home.svg", 18.0, white);
                    });
                });

                ui.add_space(18.0);
                icon_btn(ui, icons, "panels.svg", 20.0, dim);
                ui.add_space(16.0);
                icon_btn(ui, icons, "grid.svg", 20.0, dim);
                ui.add_space(16.0);
                icon_btn(ui, icons, "settings.svg", 20.0, dim);
                ui.add_space(16.0);
                icon_btn(ui, icons, "gift.svg", 20.0, dim);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
                    ui.painter()
                        .circle_filled(r.center(), 14.0, Color32::from_rgb(48, 52, 48));
                    ui.painter().circle_stroke(
                        r.center(),
                        14.0,
                        Stroke::new(1.0_f32, Color32::from_rgb(60, 65, 60)),
                    );
                    ui.painter().text(
                        r.center(),
                        egui::Align2::CENTER_CENTER,
                        "C",
                        egui::FontId::proportional(13.0),
                        white,
                    );
                });
            });
        });

    egui::SidePanel::left("openharness_drawer")
        .exact_width(250.0)
        .frame(
            egui::Frame::none()
                .fill(bg)
                .inner_margin(Margin::symmetric(14.0, 12.0))
                .stroke(Stroke::new(1.0_f32, border)),
        )
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let (r, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_stroke(
                        Rect::from_center_size(r.center(), egui::vec2(14.0, 14.0)),
                        Rounding::same(3.0),
                        Stroke::new(1.5_f32, white),
                    );
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            Pos2::new(r.center().x - 4.0, r.center().y - 2.0),
                            egui::vec2(8.0, 4.0),
                        ),
                        Rounding::same(1.0),
                        sage,
                    );
                    ui.add_space(4.0);
                    icon_ui(ui, icons, "logo.svg", 28.0, Color32::WHITE);
                    ui.label(
                        egui::RichText::new("OpenHarness")
                            .strong()
                            .size(19.0)
                            .color(white),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        icon_btn(ui, icons, "panels.svg", 16.0, muted);
                        ui.add_space(8.0);
                        icon_btn(ui, icons, "search.svg", 16.0, muted);
                    });
                });
                ui.add_space(14.0);
                let action =
                    |ui: &mut egui::Ui, icon_name: &str, label: &str, badge: Option<&str>| {
                        ui.allocate_ui(egui::vec2(ui.available_width(), 26.0), |ui| {
                            ui.horizontal_centered(|ui| {
                                icon_ui(ui, icons, icon_name, 15.0, muted);
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new(label)
                                        .size(13.0)
                                        .color(Color32::from_rgb(225, 225, 225)),
                                );
                                if let Some(text) = badge {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            egui::Frame::none()
                                                .fill(sage)
                                                .rounding(Rounding::same(8.0))
                                                .inner_margin(Margin::symmetric(6.0, 1.5))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        egui::RichText::new(text)
                                                            .size(10.5)
                                                            .strong()
                                                            .color(sage_dark),
                                                    );
                                                });
                                        },
                                    );
                                }
                            });
                        });
                        ui.add_space(2.0);
                    };
                action(ui, "edit.svg", "New chat", None);
                action(ui, "folder.svg", "New project", None);
                action(ui, "clock.svg", "Daily hours", Some("5 hrs"));
                action(ui, "wallet.svg", "Wallet", Some("1 hr"));
                action(ui, "rocket.svg", "Get 7x usage for $8", None);
                ui.add_space(12.0);
                ui.label(egui::RichText::new("Projects").size(11.5).color(dim));
                ui.add_space(6.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let mut project = |name: &str, sub: &str| {
                            let selected = state.selected_project.as_deref() == Some(name);
                            let bg = if selected {
                                Color32::from_rgb(32, 35, 32)
                            } else {
                                Color32::TRANSPARENT
                            };
                            let response = egui::Frame::none()
                                .fill(bg)
                                .rounding(Rounding::same(6.0))
                                .inner_margin(Margin::symmetric(6.0, 4.0))
                                .show(ui, |ui| {
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            icon_ui(ui, icons, "folder.svg", 14.0, muted);
                                            ui.add_space(6.0);
                                            ui.label(
                                                egui::RichText::new(name)
                                                    .size(13.0)
                                                    .strong()
                                                    .color(if selected {
                                                        white
                                                    } else {
                                                        Color32::from_rgb(215, 215, 215)
                                                    }),
                                            );
                                        });
                                        ui.horizontal(|ui| {
                                            ui.add_space(22.0);
                                            ui.label(
                                                egui::RichText::new(sub).size(11.5).color(dim),
                                            );
                                        });
                                    });
                                })
                                .response
                                .interact(egui::Sense::click());
                            if response.clicked() {
                                state.selected_project = Some(name.to_owned());
                            }
                            ui.add_space(2.0);
                        };
                        project("Victory Liner", "💬 Homepage, Navigation and...");
                        project("Experts", "💬 MT5 Gold# Scalping EA");
                        project("Smart Inventory", "💬 /impeccable shape the staff...");
                        project("LGU EducAssistance", "3 archived chats");
                        project("kondukt", "No chats yet");
                        project("prmsu-research-services", "1 archived chat");
                        project("LanxBooth", "No chats yet");
                        project("AI Trading", "2 archived chats");
                    });
                ui.add_space(4.0);
            });
        });

    egui::CentralPanel::default().frame(egui::Frame::none().fill(bg)).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(36.0);
                let (r, _) = ui.allocate_exact_size(egui::vec2(90.0, 80.0), egui::Sense::hover());
                draw_robot(ui.painter(), r, sage, sage_dark);
                ui.add_space(16.0);
                ui.label(egui::RichText::new("What can I help build?").size(24.0).strong().color(white));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - 220.0) / 2.0).max(0.0));
                    let pill = |ui: &mut egui::Ui, label: &str| {
                        egui::Frame::none().fill(Color32::from_rgb(32, 34, 32)).rounding(Rounding::same(14.0)).stroke(Stroke::new(1.0_f32, Color32::from_rgb(48, 52, 48))).inner_margin(Margin::symmetric(14.0, 6.0)).show(ui, |ui| { ui.label(egui::RichText::new(label).size(12.5).color(white)); });
                    };
                    pill(ui, "+ New project ▾"); ui.add_space(8.0); pill(ui, "💻 Local ▾");
                });
                ui.add_space(20.0);
                let card_w = ui.available_width().min(720.0);
                ui.allocate_ui_with_layout(egui::vec2(card_w, 68.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                egui::Frame::none().fill(panel_bg).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0_f32, border)).inner_margin(Margin::symmetric(16.0, 10.0)).show(ui, |ui| {
                    ui.set_width(card_w - 32.0);
                    ui.horizontal(|ui| {
                        ui.set_min_height(44.0);
                        icon_ui(ui, icons, "rocket.svg", 26.0, sage);
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Ask OpenHarness about your pipeline").strong().size(13.0).color(white));
                                ui.add_space(5.0);
                                ui.label(egui::RichText::new("Ad").size(11.0).color(dim));
                            });
                            ui.add_space(2.0);
                            ui.label(egui::RichText::new("Connect HubSpot to check contacts and deals from chat.").size(12.0).color(muted));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add(egui::Button::new(egui::RichText::new("Connect HubSpot").size(12.5).strong().color(sage_dark)).fill(sage).rounding(Rounding::same(8.0)).min_size(egui::vec2(112.0, 30.0)));
                        });
                    });
                });
                });
                ui.add_space(14.0);
                egui::Frame::none().fill(input_bg).rounding(Rounding::same(12.0)).stroke(Stroke::new(1.0_f32, Color32::from_rgb(46, 50, 46))).inner_margin(Margin::symmetric(16.0, 12.0)).show(ui, |ui| {
                    ui.set_width(card_w);
                    ui.vertical(|ui| {
                        ui.add(egui::TextEdit::multiline(&mut state.prompt).desired_rows(2).desired_width(f32::INFINITY).hint_text("Ask OpenHarness... Use / for commands").font(egui::TextStyle::Body).text_color(white).frame(false));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            icon_btn(ui, icons, "plus.svg", 18.0, muted);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let (r, _) = ui.allocate_exact_size(egui::vec2(32.0, 24.0), egui::Sense::click());
                                draw_wave(ui.painter(), r, sage, sage_dark);
                                ui.add_space(12.0);
                                icon_btn(ui, icons, "down.svg", 14.0, muted);
                                ui.add(egui::Button::new(egui::RichText::new("Powerful").size(12.0).color(muted)).fill(Color32::TRANSPARENT).frame(false));
                            });
                        });
                    });
                });
                ui.add_space(26.0);
                ui.allocate_ui(egui::vec2(card_w, 24.0), |ui| {
                    ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Start with a template").size(13.0).color(Color32::from_rgb(180, 185, 180)));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        icon_btn(ui, icons, "right.svg", 16.0, muted);
                        ui.add_space(4.0);
                        icon_btn(ui, icons, "left.svg", 16.0, dim);
                    });
                    });
                });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - card_w) / 2.0).max(0.0));
                    let mut card = |name: &str, mock: fn(&egui::Painter, Rect)| {
                        ui.vertical(|ui| {
                            let (r, response) = ui.allocate_exact_size(egui::vec2(98.0, 62.0), egui::Sense::click());
                            ui.painter().rect_filled(r, Rounding::same(8.0), if response.hovered() { Color32::from_rgb(44, 47, 44) } else { Color32::from_rgb(28, 30, 28) });
                            ui.painter().rect_stroke(r, Rounding::same(8.0), Stroke::new(1.0_f32, border));
                            mock(ui.painter(), r);
                            ui.add_space(5.0);
                            ui.label(egui::RichText::new(name).size(11.5).color(Color32::from_rgb(215, 215, 215)));
                        });
                        ui.add_space(10.0);
                    };
                    card("Landing page", mock_landing);
                    card("Store", mock_store);
                    card("Portfolio", mock_portfolio);
                    card("Study app", mock_study);
                    card("AI chat", mock_chat);
                    card("CRM", mock_crm);
                });
                ui.add_space(40.0);
            });
        });
    });
}

mod tests {
    #[test]
    fn test_default_view_state() {
        let view = super::OpenHarnessView::default();
        assert!(view.is_code_mode);
        assert_eq!(view.selected_project.as_deref(), Some("Victory Liner"));
        assert!(view.prompt.is_empty());
    }

    #[test]
    fn test_core_svg_icons_exist() {
        for icon in [
            "home.svg",
            "panels.svg",
            "grid.svg",
            "settings.svg",
            "gift.svg",
            "search.svg",
            "edit.svg",
            "folder.svg",
            "clock.svg",
            "wallet.svg",
            "up.svg",
            "down.svg",
            "left.svg",
            "right.svg",
            "close.svg",
            "minus.svg",
            "square.svg",
            "back.svg",
            "forward.svg",
            "local.svg",
            "plus.svg",
            "message.svg",
            "openharness-titlebar-64-v2.svg",
        ] {
            let path = std::path::Path::new("appicon").join(icon);
            assert!(path.exists(), "missing icon {:?}", path);
        }
    }
}
