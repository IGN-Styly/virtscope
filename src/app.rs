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
    #[serde(skip)]
    pan_offset_x: f32,
    #[serde(skip)]
    pan_offset_y: f32,
}

#[derive(PartialEq, Eq, serde::Deserialize, serde::Serialize, Clone, Copy)]
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
            pan_offset_x: 0.0,
            pan_offset_y: 0.0,
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

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Virtual Oscilloscope");

            ui.separator();

            ui.label("Waveform:");
            egui::ComboBox::from_id_salt("waveform_type")
                .selected_text(match self.waveform_type {
                    WaveformType::Sine => "Sine",
                    WaveformType::Square => "Square",
                    WaveformType::Triangle => "Triangle",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.waveform_type, WaveformType::Sine, "Sine");
                    ui.selectable_value(&mut self.waveform_type, WaveformType::Square, "Square");
                    ui.selectable_value(
                        &mut self.waveform_type,
                        WaveformType::Triangle,
                        "Triangle",
                    );
                    ui.add_space(8.0);

                    if ui.button("Reset Pan").clicked() {
                        self.pan_offset_x = 0.0;
                        self.pan_offset_y = 0.0;
                    }
                });

            ui.add_space(8.0);

            ui.label("Frequency (Hz):");
            ui.add(egui::Slider::new(&mut self.freq, 0.1..=500.0).logarithmic(true));
            ui.label(format!("{:.1}", self.freq));

            ui.add_space(8.0);

            ui.label("Zoom:");
            ui.add(egui::Slider::new(&mut self.zoom, 1.0..=10.0).logarithmic(true));
            ui.label(format!("{:.1}x", self.zoom));

            ui.add_space(8.0);

            ui.label("Amplitude (V):");
            ui.add(egui::Slider::new(&mut self.amplitude, 0.1..=200.0));
            ui.label(format!("{:.2}", self.amplitude));

            ui.add_space(8.0);

            ui.label("Time/div (ms):");
            ui.add(egui::Slider::new(&mut self.scale_div_ms, 0.1..=200.0));
            ui.label(format!("{:.2}", self.scale_div_ms));

            ui.add_space(8.0);

            ui.label("Volts/div:");
            ui.add(egui::Slider::new(&mut self.scale_div_volt, 0.1..=200.0));
            ui.label(format!("{:.2}", self.scale_div_volt));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // Draw waveform with square grid using all available space
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), ui.available_height()),
                egui::Sense::drag(),
            );

            // Handle scroll wheel for zoom
            if response.hovered() {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 {
                    // Positive scroll.y is up (zoom in), negative is down (zoom out)
                    let zoom_speed = 1.1;
                    if scroll > 0.0 {
                        self.zoom = (self.zoom * zoom_speed).min(10.0);
                    } else {
                        self.zoom = (self.zoom / zoom_speed).max(1.0);
                    }
                    ctx.request_repaint();
                }
            }

            // Handle mouse drag for panning
            if response.dragged() {
                let delta = response.drag_delta();
                self.pan_offset_x += delta.x;
                self.pan_offset_y += delta.y;
                ctx.request_repaint();
            }

            let painter = ui.painter_at(rect);

            let w = rect.width();
            let h = rect.height();
            let left = rect.left();
            let top = rect.top();

            // Fixed grid: 10 horizontal, 8 vertical divisions
            let hdivs = 10.0_f32;
            let vdivs = 8.0_f32;

            // Always use a square cell size, scaled by zoom
            let cell_size = (w / hdivs).min(h / vdivs) * self.zoom;

            // Define the screen position of (0,0): center of panel plus pan offset
            let origin_x = left + w / 2.0 + self.pan_offset_x;
            let origin_y = top + h / 2.0 + self.pan_offset_y;

            // Adjust scaling for waveform

            // Draw square grid
            let grid_color = egui::Color32::from_gray(60);
            let strong_grid_color = egui::Color32::from_gray(90);
            let stroke = egui::Stroke::new(1.0, grid_color);
            let strong_stroke = egui::Stroke::new(1.5, strong_grid_color);

            // Infinite grid: draw enough lines to fill the visible area, based on pan and zoom
            // Compute the visible range in grid coordinates, centered at (0,0) = (origin_x, origin_y)
            let min_x = ((left - origin_x) / cell_size).floor() as isize - 2;
            let max_x = ((left + w - origin_x) / cell_size).ceil() as isize + 2;
            let min_y = ((top - origin_y) / cell_size).floor() as isize - 2;
            let max_y = ((top + h - origin_y) / cell_size).ceil() as isize + 2;

            // Vertical grid lines (x = 0 is the y-axis)
            for i in min_x..=max_x {
                let x = origin_x + (i as f32) * cell_size;
                let s = if i == 0 { &strong_stroke } else { &stroke };
                painter.line_segment([egui::pos2(x, top), egui::pos2(x, top + h)], *s);

                // Minor increment ticks along the main X axis (center horizontal line)
                if i == 0 {
                    let minor_ticks = 10;
                    let minor_tick_len = cell_size * 0.10;
                    let major_tick_len = cell_size * 0.22;
                    let tick_color = egui::Color32::from_gray(140);
                    for div in min_y..=max_y {
                        let div_top = origin_y + (div as f32) * cell_size;
                        // Major tick at the division, but skip if at axis (0,0) to avoid double-drawing
                        if !(i == 0 && div == 0) {
                            painter.line_segment(
                                [
                                    egui::pos2(x - major_tick_len / 2.0, div_top),
                                    egui::pos2(x + major_tick_len / 2.0, div_top),
                                ],
                                egui::Stroke::new(1.5, tick_color),
                            );
                        }
                        // Minor ticks between divisions
                        for m in 1..minor_ticks {
                            let frac = m as f32 / minor_ticks as f32;
                            let y_tick = div_top + frac * cell_size;
                            painter.line_segment(
                                [
                                    egui::pos2(x - minor_tick_len / 2.0, y_tick),
                                    egui::pos2(x + minor_tick_len / 2.0, y_tick),
                                ],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                    }
                }

                // Draw small ticks only on the main X axis (center horizontal line)
                if i == 0 {
                    let y = origin_y;
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
            // Horizontal grid lines (y = 0 is the x-axis)
            for j in min_y..=max_y {
                let y = origin_y + (j as f32) * cell_size;
                let s = if j == 0 { &strong_stroke } else { &stroke };
                painter.line_segment([egui::pos2(left, y), egui::pos2(left + w, y)], *s);

                // Minor increment ticks along the main Y axis (center vertical line)
                if j == 0 {
                    let minor_ticks = 10;
                    let minor_tick_len = cell_size * 0.10;
                    let major_tick_len = cell_size * 0.22;
                    let tick_color = egui::Color32::from_gray(140);
                    for div in min_x..=max_x {
                        let div_left = origin_x + (div as f32) * cell_size;
                        // Major tick at the division, but skip if at axis (0,0) to avoid double-drawing
                        if !(j == 0 && div == 0) {
                            painter.line_segment(
                                [
                                    egui::pos2(div_left, y - major_tick_len / 2.0),
                                    egui::pos2(div_left, y + major_tick_len / 2.0),
                                ],
                                egui::Stroke::new(1.5, tick_color),
                            );
                        }
                        // Minor ticks between divisions
                        for m in 1..minor_ticks {
                            let frac = m as f32 / minor_ticks as f32;
                            let x_tick = div_left + frac * cell_size;
                            painter.line_segment(
                                [
                                    egui::pos2(x_tick, y - minor_tick_len / 2.0),
                                    egui::pos2(x_tick, y + minor_tick_len / 2.0),
                                ],
                                egui::Stroke::new(1.0, tick_color),
                            );
                        }
                    }
                }

                // Draw small ticks only on the main Y axis (center vertical line)
                if j == 0 {
                    let x = origin_x;
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
            // No border rectangle needed for infinite grid

            // Draw waveform
            let volts_per_div = self.scale_div_volt;
            let freq = self.freq;
            let amplitude = self.amplitude;
            let ms_per_div = self.scale_div_ms;

            // Calculate number of points based on screen width (like infinite grid)
            // Use every 2 pixels for good performance while maintaining smooth curves
            let pixel_step = 2.0;
            let visible_points = (w / pixel_step) as usize;
            let mut points: Vec<egui::Pos2> = Vec::with_capacity(visible_points);

            // Calculate the visible x range in screen coordinates
            let x_start = left;
            let x_end = left + w;

            for i in 0..visible_points {
                // Calculate screen x position
                let x = x_start + (i as f32) * pixel_step;
                // Calculate distance from (0,0) marker in pixels
                let dx = x - origin_x;
                // Convert to grid divisions
                let dx_grid = dx / cell_size;
                // Convert to time in ms (center is t=0)
                let t_ms = dx_grid * ms_per_div;
                // Convert ms to seconds
                let t = t_ms / 1000.0;
                // Calculate phase for this time
                let phase = 2.0 * std::f32::consts::PI * freq * t;
                // Evaluate waveform at this phase
                let v = match self.waveform_type {
                    WaveformType::Sine => amplitude * phase.sin(),
                    WaveformType::Square => {
                        // Square wave: positive when sin of phase is positive
                        if phase.sin() >= 0.0 {
                            amplitude
                        } else {
                            -amplitude
                        }
                    }
                    WaveformType::Triangle => {
                        // Triangle wave using asin of sin to create triangle shape
                        let triangle_phase = (2.0 * phase.sin()).clamp(-1.0, 1.0).asin();
                        amplitude * (2.0 / std::f32::consts::PI) * triangle_phase
                    }
                };
                let y = origin_y - (v / volts_per_div) * cell_size;
                points.push(egui::pos2(x, y));
            }

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
            let phase = 2.0 * std::f32::consts::PI * freq * t;
            let v = match self.waveform_type {
                WaveformType::Sine => amp * phase.sin(),
                WaveformType::Square => {
                    // Square wave: positive when in first half of period
                    let period_pos = (phase / (2.0 * std::f32::consts::PI)).rem_euclid(1.0);
                    if period_pos < 0.5 { amp } else { -amp }
                }
                WaveformType::Triangle => {
                    // Triangle wave: sawtooth that goes up and down
                    let period_pos = (phase / (2.0 * std::f32::consts::PI)).rem_euclid(1.0);
                    let triangle_val = if period_pos < 0.5 {
                        4.0 * period_pos - 1.0
                    } else {
                        3.0 - 4.0 * period_pos
                    };
                    amp * triangle_val
                }
            };
            self.waveform[i] = v;
        }
    }
}
