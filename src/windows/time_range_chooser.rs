use chrono::prelude::*;
use chrono::{Date, NaiveTime, Utc};
use crossbeam::channel::{Receiver, Sender};
use egui::{Ui, Window};
use tracing::{error, info, warn};

use crate::{
    netstrat::{
        bounds::{Bounds, BoundsSet},
        props::Props,
    },
    sources::binance::Interval,
    widgets::TimeInput,
};

use super::AppWindow;

pub struct TimeRangeChooser {
    symbol: String,
    time_start_input: TimeInput,
    time_end_input: TimeInput,
    valid: bool,
    visible: bool,
    date_start: Date<Utc>,
    date_end: Date<Utc>,
    interval: Interval,
    symbol_sub: Receiver<String>,
    props_sub: Receiver<Props>,
    props_pub: Sender<Props>,
    export_pub: Sender<Props>,
}

impl TimeRangeChooser {
    pub fn new(
        visible: bool,
        symbol_sub: Receiver<String>,
        props_pub: Sender<Props>,
        props_sub: Receiver<Props>,
        export_pub: Sender<Props>,
        props: Props,
    ) -> Self {
        Self {
            symbol: String::new(),
            symbol_sub,
            valid: true,
            visible,
            props_pub,
            props_sub,
            export_pub,
            date_start: props.date_start,
            date_end: props.date_end,
            interval: props.interval,
            time_start_input: TimeInput::new(
                props.time_start.hour(),
                props.time_start.minute(),
                props.time_start.second(),
            ),
            time_end_input: TimeInput::new(
                props.time_end.hour(),
                props.time_end.minute(),
                props.time_end.second(),
            ),
        }
    }
}

impl TimeRangeChooser {
    fn parse_props(
        time_start_opt: Option<NaiveTime>,
        time_end_opt: Option<NaiveTime>,
        date_start: Date<Utc>,
        date_end: Date<Utc>,
        interval: Interval,
    ) -> Option<Props> {
        let time_start: NaiveTime;
        match time_start_opt {
            Some(time) => {
                time_start = time;
            }
            None => {
                return None;
            }
        }

        let time_end: NaiveTime;
        match time_end_opt {
            Some(time) => {
                time_end = time;
            }
            None => {
                return None;
            }
        }
        let mut p = Props {
            date_start,
            date_end,
            time_start,
            time_end,
            interval,
            bounds: BoundsSet::new(vec![]),
            limit: 1000,
        };
        p.bounds = BoundsSet::new(vec![Bounds(
            p.start_time().timestamp_millis(),
            p.end_time().timestamp_millis(),
        )]);

        Some(p)
    }

    fn unpack_props(&mut self, p: &Props) {
        info!("unpacking props...");

        self.date_start = p.date_start;
        self.date_end = p.date_end;

        let time_start = p.time_start;
        self.time_start_input =
            TimeInput::new(time_start.hour(), time_start.minute(), time_start.second());

        let time_end = p.time_end;
        self.time_end_input = TimeInput::new(time_end.hour(), time_end.minute(), time_end.second());

        info!("props unpacked and applied");
    }
}

impl AppWindow for TimeRangeChooser {
    fn toggle_btn(&mut self, ui: &mut Ui) {
        if ui.button("Props").clicked() {
            self.visible = !self.visible
        }
    }

    fn show(&mut self, ui: &mut Ui) {
        let symbol_wrapped = self
            .symbol_sub
            .recv_timeout(std::time::Duration::from_millis(1));

        match symbol_wrapped {
            Ok(symbol) => {
                info!("received symbol: {symbol}");
                self.symbol = symbol;
            }
            Err(_) => {}
        }

        let props_wrapped = self
            .props_sub
            .recv_timeout(std::time::Duration::from_millis(1));

        match props_wrapped {
            Ok(props) => {
                info!("received props: {props:?}");
                self.unpack_props(&props);
            }
            Err(_) => {}
        }

        Window::new(self.symbol.to_string())
            .open(&mut self.visible)
            .drag_bounds(ui.max_rect())
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.collapsing("Time Period", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui_extras::DatePickerButton::new(&mut self.date_start)
                                .id_source("datepicker_start"),
                        );
                        ui.label("Date Start");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.add(
                            egui_extras::DatePickerButton::new(&mut self.date_end)
                                .id_source("datepicker_end"),
                        );
                        ui.label("Date End");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.add(&mut self.time_start_input);
                        ui.label("Time Start");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.add(&mut self.time_end_input);
                        ui.label("Time End");
                    });
                });
                ui.collapsing("Interval", |ui| {
                    egui::ComboBox::from_label("pick data interval")
                        .selected_text(format!("{:?}", self.interval))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.interval, Interval::Day, "Day");
                            ui.selectable_value(&mut self.interval, Interval::Hour, "Hour");
                            ui.selectable_value(&mut self.interval, Interval::Minute, "Minute");
                        });
                });

                ui.add_space(5f32);

                ui.horizontal(|ui| {
                    if ui.button("show").clicked() {
                        let props = TimeRangeChooser::parse_props(
                            self.time_start_input.get_time(),
                            self.time_end_input.get_time(),
                            self.date_start,
                            self.date_end,
                            self.interval,
                        );
                        match props {
                            Some(props) => {
                                if props.is_valid() {
                                    let send_result = self.props_pub.send(props.clone());
                                    match send_result {
                                        Ok(_) => {
                                            info!("sent props for show: {props:?}");
                                        }
                                        Err(err) => {
                                            error!("failed to send props for show: {err}");
                                        }
                                    }
                                } else {
                                    warn!("invalid props");
                                    self.valid = false;
                                }
                            }
                            None => {
                                error!("failed to parse props");
                                self.valid = false;
                            }
                        }
                    }

                    if ui.button("export").clicked() {
                        let props = TimeRangeChooser::parse_props(
                            self.time_start_input.get_time(),
                            self.time_end_input.get_time(),
                            self.date_start,
                            self.date_end,
                            self.interval,
                        );
                        match props {
                            Some(props) => {
                                if props.is_valid() {
                                    let send_result = self.export_pub.send(props.clone());
                                    match send_result {
                                        Ok(_) => {
                                            info!("sent props for export: {props:?}");
                                        }
                                        Err(err) => {
                                            error!("failed to send props for export: {err}");
                                        }
                                    }
                                } else {
                                    warn!("invalid props");
                                    self.valid = false;
                                }
                            }
                            None => {
                                error!("failed to parse props");
                                self.valid = false;
                            }
                        }
                    };
                });

                if !self.valid {
                    ui.label("invalid time format or start > end");
                }
            });
    }
}
