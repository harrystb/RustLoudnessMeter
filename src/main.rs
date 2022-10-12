use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;
use std::collections::VecDeque;
use eframe::{egui, Frame, Storage};
use egui::widgets::Label;
use egui::{Color32, Context, Rgba, TopBottomPanel, Ui, Vec2, Visuals};
use crate::egui::TextStyle;
use cpal::{Stream, traits::StreamTrait};
use cpal::traits::{HostTrait, DeviceTrait};
use eframe::egui::plot::{Line, Plot, PlotPoints};
use rustfft::{FftPlanner, num_complex::Complex};
use rustfft::num_complex::ComplexFloat;

mod lufs;
use lufs::LUFSCalculator;

fn main() {
    let mut options = eframe::NativeOptions::default();
    options.min_window_size = Some((150.0,370.0).into());
    //options.max_window_size = Some((150.0,370.0).into());
    let mut app = LoudnessApp::default();

    let host = cpal::default_host();

    let device = host.default_input_device().expect("no input device found");

    let supported_config = device.supported_input_configs().expect("Error getting the device config").next().expect("no supported device config").with_max_sample_rate();

    let (tx_chan, rx_chan) = channel();
    let (tx_chan2, rx_chan2) = channel();

    LUFSCalculator::start(rx_chan, tx_chan2, supported_config.sample_rate().0);

    app.rx_channel = Some(rx_chan2);
    app.sample_rate = supported_config.sample_rate().0;

    let mut count = 0;
    let mut sum = 0.;

    app.stream = Some(device.build_input_stream(
        &supported_config.into(),
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            for d in data {
                match tx_chan.send(*d) {
                    Ok(_) => (),
                    Err(_) => (),
                };
            }
        },
        move |err| {
            eprintln!("Encountered an error {:?}", err);
        }
    ).expect("unable to open stream"));

    if let Some(stream) = &app.stream {
        stream.play().expect("unable to start the stream");
    }

    eframe::run_native(
        "Loudness Meter",
        options,
        Box::new(|_cc| Box::new(app)),
    );

}

struct LoudnessApp {
    level_history: VecDeque<f32>,
    level: f32,
    rx_channel: Option<Receiver<f32>>,
    stream: Option<Stream>,
    sample_rate: u32,
}

impl Default for LoudnessApp {
    fn default() -> Self {
        // we want history to be last minute - with 100ms per sample - 600 samples total
        let mut level_history = VecDeque::from([-60.; 600]);
        Self{level_history , level: -60., rx_channel: None, stream: None, sample_rate: 0}
    }
}

const WHITE_COLOUR: Color32 = Color32::from_rgb(255, 255, 255);
const RED_COLOUR: Color32 = Color32::from_rgb(255, 0, 0);
const HEADER_FOOTER_BG_COLOUR: Color32 = Color32::from_rgb(60,63,65,);

impl eframe::App for LoudnessApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        ctx.request_repaint_after(Duration::from_micros(100));
        match &self.rx_channel {
            Some(rx) => match rx.try_recv() {
                Ok(v) => {
                    self.level_history.pop_front();
                    self.level_history.push_back(v);
                    self.level = v;
                },
                Err(e) => (),
            }
            None => self.level = -60.,
        };
        let meter_portion = self.level / -60.;

        self.render_header(ctx);
        self.render_footer(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            let text_height = ui.text_style_height(&TextStyle::Body);
            let vert_h = ui.available_height() - 10. - ui.spacing().item_spacing.y * 2.;
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.horizontal(|ui| {
                    ui.style_mut().spacing.item_spacing.x = 0.0;
                    ui.allocate_ui_with_layout((30.0, vert_h).into(), egui::Layout::top_down(egui::Align::RIGHT), |ui| {
                        ui.set_width(40.0);
                        ui.style_mut().spacing.item_spacing = (0., 0.).into();
                        ui.label("0dB");
                        for i in 1..21 {
                            ui.add_space((vert_h - 21.0 * text_height) / 20.0);
                            ui.label(format!("{}dB", { i as f32 } * -3.0));
                        }
                    });
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.set_width(4.0);
                        ui.set_height(vert_h);
                        for i in 0..21 {
                            let mut rect = ui.max_rect();
                            rect.set_top(rect.top() + 0.5 * text_height + { i as f32 } * (vert_h - text_height - 2.) / 20.);
                            rect.set_bottom(rect.top() + 2.0);
                            ui.painter().rect_filled(rect, 0.0, WHITE_COLOUR);
                        }
                    });
                    ui.vertical(|ui| {
                        ui.set_width(2.0);
                        ui.set_height(vert_h);
                        let mut rect = ui.max_rect();
                        rect.set_bottom(rect.bottom() - 0.5 * text_height);
                        rect.set_top(rect.top() + 0.5 * text_height);
                        ui.painter().rect_filled(rect, 0.0, WHITE_COLOUR);
                    });
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.set_width(10.0);
                        ui.set_height(vert_h);
                        let mut white_box_rect = ui.max_rect().clone();
                        white_box_rect.set_bottom(white_box_rect.bottom() - 0.5 * text_height);
                        if self.level > -20. {
                            white_box_rect.set_top(white_box_rect.top() + 20. / 60. * vert_h + 0.5 * text_height);
                            let mut red_box_rect = ui.max_rect().clone();
                            red_box_rect.set_bottom(red_box_rect.top() + 20. / 60. * vert_h + 0.5 * text_height);
                            red_box_rect.set_top(red_box_rect.top() + text_height + (-20. - self.level) / -60. * vert_h);
                            ui.painter().rect_filled(red_box_rect, 0.0, RED_COLOUR);
                        } else {
                            white_box_rect.set_top(white_box_rect.top() + meter_portion * vert_h + 0.5 * text_height);
                        }
                        ui.painter().rect_filled(white_box_rect, 0.0, WHITE_COLOUR);
                    });
                    ui.add_space(2.0);
                    ui.vertical(|ui| {
                        ui.set_width(2.0);
                        ui.set_height(vert_h);
                        let mut rect = ui.max_rect();
                        rect.set_bottom(rect.bottom() - 0.5 * text_height);
                        rect.set_top(rect.top() + 0.5 * text_height);
                        ui.painter().rect_filled(rect, 0.0, WHITE_COLOUR);
                    });
                    ui.vertical(|ui| {
                        ui.set_width(4.0);
                        ui.set_height(vert_h);
                        for i in 0..21 {
                            let mut rect = ui.max_rect();
                            rect.set_top(rect.top() + 0.5 * text_height + { i as f32 } * (vert_h - text_height - 2.) / 20.);
                            rect.set_bottom(rect.top() + 2.0);
                            ui.painter().rect_filled(rect, 0.0, WHITE_COLOUR);
                        }
                    });
                    ui.add_space(2.0);
                    ui.allocate_ui_with_layout((40.0, vert_h).into(), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.set_width(40.0);
                        ui.style_mut().spacing.item_spacing = (0., 0.).into();
                        ui.label("0dB");
                        for i in 1..21 {
                            ui.add_space((vert_h - 21.0 * text_height) / 20.0);
                            ui.label(format!("{}dB", { i as f32 } * -3.0));
                        }
                    });
                });
                ui.vertical(|ui| {
                    Plot::new("level_history").show_background(false).show_axes([false, false]).show(ui, |plot_ui| {
                        plot_ui.line(Line::new(self.level_history.iter().enumerate().map(|(i, v)| [(-599. + (i as f64)) / 10., *v as f64]).collect::<Vec<[f64;2]>>()));
                    });
                });
            });
            ui.add_space(5.0);
        });
    }
}

impl LoudnessApp {
    fn render_footer(&self, ctx: &Context) {
        let mut frame = egui::Frame::default();
        frame.fill = HEADER_FOOTER_BG_COLOUR;
        TopBottomPanel::bottom("footer").frame(frame).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.add(Label::new(format!("{}", self.sample_rate)));
                ui.add(Label::new("Harrison St Baker"));
                ui.add_space(5.0);
            });
        });
    }
    fn render_header(&self, ctx: &Context) {
        let mut frame = egui::Frame::default();
        frame.fill = HEADER_FOOTER_BG_COLOUR;
        TopBottomPanel::top("header").frame(frame).show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(5.0);
                ui.heading("Loudness Meter");
                ui.add_space(5.0);
            });
        });
    }

}
