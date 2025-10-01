/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    #[serde(skip)] // This how you opt-out of serialization of a field
    value: f32,
    freq: f32,
    amplitude: f32,
    scale_div_volt: f32,
    scale_div_ms: f32,

    #[serde(skip)]
    running: bool,
    #[serde(skip)]
    waveform: Vec<f32>,
    #[serde(skip)]
    phase: f64,
    #[serde(skip)]
    waveform_type: WaveformType,
    #[serde(skip)]
    zoom: f32,
}

#[derive(PartialEq, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub enum WaveformType {
    Sine,
    Square,
    Triangle,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            amplitude: 5.0,
            freq: 250.0,
            scale_div_ms: 1.0,
            scale_div_volt: 1.0,
            running: true,
            waveform: vec![0.0; 512],
            phase: 0.0,
            waveform_type: WaveformType::Sine,
            zoom: 1.0,
        }
    }
}

impl TemplateApp {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }
}

impl eframe::App for TemplateApp {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update waveform if running
        if self.running {
            self.generate_waveform();
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }
                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Virtual Oscilloscope");

            ui.horizontal(|ui| {
                ui.label("Waveform:");
                egui::ComboBox::from_id_salt("waveform_type")
                    .selected_text(match self.waveform_type {
                        WaveformType::Sine => "Sine",
                        WaveformType::Square => "Square",
                        WaveformType::Triangle => "Triangle",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.waveform_type, WaveformType::Sine, "Sine");
                        ui.selectable_value(
                            &mut self.waveform_type,
                            WaveformType::Square,
                            "Square",
                        );
                        ui.selectable_value(
                            &mut self.waveform_type,
                            WaveformType::Triangle,
                            "Triangle",
                        );
                    });
            });

            ui.horizontal(|ui| {
                ui.label("Frequency (Hz):");
                ui.add(egui::Slider::new(&mut self.freq, 0.1..=1000.0).logarithmic(true));
                ui.label(format!("{:.1}", self.freq));
            });

            ui.horizontal(|ui| {
                ui.label("Zoom:");
                ui.add(egui::Slider::new(&mut self.zoom, 1.0..=10.0).logarithmic(true));
                ui.label(format!("{:.1}x", self.zoom));
            });

            ui.horizontal(|ui| {
                ui.label("Amplitude (V):");
                ui.add(egui::Slider::new(&mut self.amplitude, 0.1..=20.0));
                ui.label(format!("{:.2}", self.amplitude));
            });

            ui.horizontal(|ui| {
                ui.label("Time/div (ms):");
                ui.add(egui::Slider::new(&mut self.scale_div_ms, 0.1..=200.0));
                ui.label(format!("{:.2}", self.scale_div_ms));
            });

            ui.horizontal(|ui| {
                ui.label("Volts/div:");
                ui.add(egui::Slider::new(&mut self.scale_div_volt, 0.1..=20.0));
                ui.label(format!("{:.2}", self.scale_div_volt));
            });

            ui.separator();

            // Draw waveform with square grid and padding
            let padding = 12.0;
            let (rect, _response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), 900.0),
                egui::Sense::hover(),
            );
            let painter = ui.painter_at(rect);

            let n = self.waveform.len();
            let w = rect.width() - 2.0 * padding;
            let h = rect.height() - 2.0 * padding;
            let left = rect.left() + padding;
            let top = rect.top() + padding;

            // Fixed grid: 10 horizontal, 8 vertical divisions
            let hdivs = 10.0_f32;
            let vdivs = 8.0_f32;

            // Apply zoom globally to cell_size (grid, ticks, waveform, etc.)
            let base_cell_size = w.min(h / vdivs * hdivs) / hdivs;
            let cell_size = base_cell_size * self.zoom; // scale everything by zoom

            // Recompute cell_size to fit both axes, center grid
            let grid_width = cell_size * hdivs;
            let grid_height = cell_size * vdivs;
            let grid_left = left + (w - grid_width) / 2.0;
            let grid_top = top + (h - grid_height) / 2.0;
            let grid_right = grid_left + grid_width;
            let grid_bottom = grid_top + grid_height;
            let mid_y = grid_top + (vdivs / 2.0) * cell_size;

            // Adjust scaling for waveform
            let volts_per_div = self.scale_div_volt as f32;
            let time_per_div = self.scale_div_ms as f32 / 1000.0; // ms to s
            let total_time = time_per_div * hdivs;

            // Draw square grid
            let grid_color = egui::Color32::from_gray(60);
            let strong_grid_color = egui::Color32::from_gray(90);
            let stroke = egui::Stroke::new(1.0, grid_color);
            let strong_stroke = egui::Stroke::new(1.5, strong_grid_color);

            // Vertical grid lines
            // Vertical grid lines
            for i in 0..=hdivs as usize {
                let x = grid_left + (i as f32) * cell_size;
                let s = if i == (hdivs / 2.0).round() as usize {
                    &strong_stroke
                } else {
                    &stroke
                };
                painter.line_segment([egui::pos2(x, grid_top), egui::pos2(x, grid_bottom)], *s);

                // Minor increment ticks along the main X axis (center horizontal line)
                if i == (hdivs / 2.0).round() as usize {
                    let y = mid_y;
                    let tick_len = cell_size * 0.12;
                    let minor_ticks = 10;
                    let tick_color = egui::Color32::from_rgb(120, 180, 255); // subtle blue
                    for div in 0..(vdivs as usize) {
                        let div_top = grid_top + div as f32 * cell_size;
                        for m in 1..minor_ticks {
                            let frac = m as f32 / minor_ticks as f32;
                            if frac == 0.0 {
                                continue;
                            }
                            let y_tick = div_top + frac * cell_size;
                            painter.line_segment(
                                [
                                    egui::pos2(x - tick_len / 2.0, y_tick),
                                    egui::pos2(x + tick_len / 2.0, y_tick),
                                ],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                    }
                }

                // Draw small ticks only on the main X axis (center horizontal line)
                if (hdivs / 2.0).round() as usize == hdivs as usize / 2 {
                    let y = mid_y;
                    let tick_len = cell_size * 0.25;
                    painter.line_segment(
                        [
                            egui::pos2(x, y - tick_len / 2.0),
                            egui::pos2(x, y + tick_len / 2.0),
                        ],
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                    );
                }
            }
            // Horizontal grid lines
            for i in 0..=vdivs as usize {
                let y = grid_top + (i as f32) * cell_size;
                let s = if i == (vdivs / 2.0).round() as usize {
                    &strong_stroke
                } else {
                    &stroke
                };
                painter.line_segment([egui::pos2(grid_left, y), egui::pos2(grid_right, y)], *s);

                // Minor increment ticks along the main Y axis (center vertical line)
                if i == (vdivs / 2.0).round() as usize {
                    let x = grid_left + (hdivs / 2.0) * cell_size;
                    let tick_len = cell_size * 0.12;
                    let minor_ticks = 10;
                    let tick_color = egui::Color32::from_rgb(120, 180, 255); // subtle blue
                    for div in 0..(hdivs as usize) {
                        let div_left = grid_left + div as f32 * cell_size;
                        for m in 1..minor_ticks {
                            let frac = m as f32 / minor_ticks as f32;
                            if frac == 0.0 {
                                continue;
                            }
                            let x_tick = div_left + frac * cell_size;
                            painter.line_segment(
                                [
                                    egui::pos2(x_tick, y - tick_len / 2.0),
                                    egui::pos2(x_tick, y + tick_len / 2.0),
                                ],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                    }
                }

                // Draw small ticks only on the main Y axis (center vertical line)
                if (vdivs / 2.0).round() as usize == vdivs as usize / 2 {
                    let x = grid_left + (hdivs / 2.0) * cell_size;
                    let tick_len = cell_size * 0.25;
                    painter.line_segment(
                        [
                            egui::pos2(x - tick_len / 2.0, y),
                            egui::pos2(x + tick_len / 2.0, y),
                        ],
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                    );
                }
            }

            // Draw border
            painter.rect_stroke(
                egui::Rect::from_min_max(
                    egui::pos2(grid_left as f32, grid_top as f32),
                    egui::pos2(grid_right as f32, grid_bottom as f32),
                ),
                0.0,
                egui::Stroke::new(1.5, egui::Color32::DARK_GRAY),
                egui::StrokeKind::Middle,
            );

            // Draw waveform
            let volts_per_div = self.scale_div_volt;
            let time_per_div = self.scale_div_ms / 1000.0;
            let hdivs = 10.0;
            let n = self.waveform.len();
            let zoom = self.zoom.max(1.0);
            let center = n / 2;
            let mut visible = (n as f32 / zoom).round() as usize;
            if visible % 2 == 0 {
                visible = visible.saturating_sub(1);
            }
            let start = center.saturating_sub(visible / 2);
            let end = (start + visible).min(n);

            let width = cell_size * hdivs;
            // Center waveform at (0,0): index 0 is at grid center, waveform extends left and right
            let grid_center_x = grid_left + (hdivs / 2.0) * cell_size;
            let half_visible = visible / 2;
            let center_index = n / 2;
            let points: Vec<egui::Pos2> = (0..visible)
                .filter_map(|j| {
                    let i = center_index as isize + (j as isize - half_visible as isize);
                    if i >= 0 && (i as usize) < n {
                        let v = self.waveform[i as usize];
                        let x = grid_center_x
                            + (j as f32 - half_visible as f32) * (width / (visible as f32 - 1.0));
                        let y = mid_y - (v / volts_per_div) * cell_size;
                        Some(egui::pos2(x, y))
                    } else {
                        None
                    }
                })
                .collect();

            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(2.0, egui::Color32::YELLOW),
            ));

            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                powered_by_egui_and_eframe(ui);
                egui::warn_if_debug_build(ui);
            });
        });
    }
}

fn powered_by_egui_and_eframe(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}

// --- Oscilloscope waveform generation ---
impl TemplateApp {
    fn generate_waveform(&mut self) {
        let n = self.waveform.len();
        let hdivs = 10.0;
        let time_per_div = self.scale_div_ms / 1000.0;
        let full_time = hdivs * time_per_div;
        let dt = full_time / (n as f32 - 1.0);
        let freq = self.freq;
        let amp = self.amplitude;

        let center = n as isize / 2;
        for i in 0..n {
            let t = (i as isize - center) as f32 * dt;
            let v = match self.waveform_type {
                WaveformType::Sine => amp * (2.0 * std::f32::consts::PI * freq * t).sin(),
                WaveformType::Square => {
                    if ((freq * t).fract()) < 0.5 {
                        amp
                    } else {
                        -amp
                    }
                }
                WaveformType::Triangle => {
                    let frac = (freq * t).fract();
                    amp * (4.0 * (frac - 0.5)).abs() - amp
                }
            };
            self.waveform[i] = v;
        }
    }
}
