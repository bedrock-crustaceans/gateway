use crate::network::source::Source;
use crate::ui::filter::{Filter, PacketInfo, Rule};
use crate::ui::inspector::{InspectorTab, Inspection};
use crate::ui::theme;
use bedrock::network::motd::BedrockMOTD;
use bedrock::protocol::PacketDyn;
use chrono::{DateTime, Local};
use egui::{Align, Label, Layout, Panel, RichText, ScrollArea, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use std::net::SocketAddr;

pub struct PacketEntry {
    pub timestamp: DateTime<Local>,
    pub source: Source,
    pub addr: SocketAddr,
    pub frame: Vec<u8>,
    pub packet: Box<dyn PacketDyn>,
}

impl PacketEntry {
    pub fn name(&self) -> &'static str {
        self.packet.meta().name
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum View {
    Packets,
    Log,
}

pub struct Capture {
    pub entries: Vec<PacketEntry>,
    pub filter: Filter,
    pub paused: bool,
    pub follow: bool,
    pub motd: String,
    pub events: Vec<String>,
    pub inspection: Option<Inspection>,
    pub view: View,
    pub inspector_tab: InspectorTab,
}

impl Default for Capture {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            filter: Filter::default(),
            paused: false,
            follow: true,
            motd: String::new(),
            events: Vec::new(),
            inspection: None,
            view: View::Packets,
            inspector_tab: InspectorTab::default(),
        }
    }
}

impl Capture {
    pub fn push(&mut self, packet: Box<dyn PacketDyn>, frame: Vec<u8>, source: Source, addr: SocketAddr) {
        if self.paused {
            return;
        }

        self.entries.push(PacketEntry {
            timestamp: Local::now(),
            source,
            addr,
            frame,
            packet,
        });
    }

    pub fn set_motd(&mut self, raw: &[u8]) {
        self.motd = match BedrockMOTD::parse(raw) {
            Ok(motd) => format!(
                "{}  ·  {} (protocol {})  ·  {}/{} players",
                motd.name, motd.version, motd.protocol, motd.player_count, motd.player_max
            ),
            Err(_) => String::from_utf8_lossy(raw).into_owned(),
        };
    }

    pub fn log(&mut self, msg: String) {
        self.events.push(format!("{}  {msg}", Local::now().format("%H:%M:%S")));
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.events.clear();
        self.inspection = None;
    }

    fn select(&mut self, index: usize) {
        self.follow = false;
        self.inspection = Some(Inspection::new(index, &self.entries[index]));
    }

    fn visible(&mut self) -> Vec<usize> {
        let filter = &mut self.filter;
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                let info = PacketInfo {
                    name: e.name(),
                    id: e.packet.id(),
                    source: e.source,
                    addr: e.addr,
                    size: e.frame.len(),
                };
                filter.matches(&info)
            })
            .map(|(i, _)| i)
            .collect()
    }
}

enum RowAction {
    Select(usize),
    Rule(&'static str, Rule),
    OnlyClient(SocketAddr),
    HideClient(SocketAddr),
    ResetFilter,
}

pub fn main_view(ui: &mut Ui, capture: &mut Capture) {
    let visible = capture.visible();
    let packets_label = if visible.len() == capture.entries.len() {
        format!("Packets  {}", capture.entries.len())
    } else {
        format!("Packets  {} / {}", visible.len(), capture.entries.len())
    };
    let tabs = [
        (View::Packets, packets_label),
        (View::Log, format!("Log  {}", capture.events.len())),
    ];
    theme::tabs(ui, &mut capture.view, &tabs);
    ui.add_space(6.);

    match capture.view {
        View::Packets => packet_table(ui, capture, visible),
        View::Log => log_view(ui, capture),
    }
}

fn packet_table(ui: &mut Ui, capture: &mut Capture, visible: Vec<usize>) {
    let selected = capture.inspection.as_ref().map(|i| i.index);
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace) + 8.;
    let mut action = None;

    if visible.is_empty() {
        ui.add_space(40.);
        ui.vertical_centered(|ui| {
            let hint = if capture.entries.is_empty() { "No packets yet. Start the proxy and connect a client." } else { "No packets match the filter." };
            ui.label(RichText::new(hint).color(theme::TEXT_DIM));
        });
        return;
    }

    TableBuilder::new(ui)
        .striped(true)
        .sense(Sense::click())
        .stick_to_bottom(capture.follow)
        .auto_shrink(false)
        .cell_layout(Layout::left_to_right(Align::Center))
        .column(Column::exact(56.))
        .column(Column::exact(92.))
        .column(Column::exact(56.))
        .column(Column::initial(280.).at_least(120.).resizable(true).clip(true))
        .column(Column::exact(72.))
        .column(Column::remainder().clip(true))
        .header(22., |mut header| {
            for title in ["#", "Time", "Dir", "Packet", "Size", "Client"] {
                header.col(|ui| {
                    ui.label(RichText::new(title).small().strong().color(theme::TEXT_DIM));
                });
            }
        })
        .body(|body| {
            body.rows(row_height, visible.len(), |mut row| {
                let index = visible[row.index()];
                let entry = &capture.entries[index];
                row.set_selected(selected == Some(index));

                let (dir, color) = theme::direction(entry.source);

                row.col(|ui| {
                    ui.label(RichText::new((index + 1).to_string()).monospace().color(theme::TEXT_DIM));
                });
                row.col(|ui| {
                    ui.label(RichText::new(entry.timestamp.format("%H:%M:%S%.3f").to_string()).monospace().color(theme::TEXT_DIM));
                });
                row.col(|ui| {
                    theme::badge(ui, &dir, color);
                });
                row.col(|ui| {
                    let unknown = entry.name() == "UnknownPacket";
                    let name = if unknown { format!("Unknown ({})", entry.packet.id()) } else { entry.name().to_string() };
                    ui.add(Label::new(RichText::new(name).color(if unknown { theme::DANGER } else { theme::TEXT })).selectable(false).truncate());
                });
                row.col(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format_size(entry.frame.len())).monospace().color(theme::TEXT_DIM));
                    });
                });
                row.col(|ui| {
                    ui.label(RichText::new(entry.addr.to_string()).monospace().color(theme::TEXT_DIM));
                });

                let response = row.response();
                if response.clicked() {
                    action = Some(RowAction::Select(index));
                }
                response.context_menu(|ui| {
                    let name = entry.name();
                    let mut item = |ui: &mut Ui, text: String, value: RowAction| {
                        if ui.button(text).clicked() {
                            action = Some(value);
                        }
                    };
                    item(ui, format!("Hide {name}"), RowAction::Rule(name, Rule::Hide));
                    item(ui, format!("Only show {name}"), RowAction::Rule(name, Rule::Only));
                    ui.separator();
                    item(ui, format!("Only show {}", entry.addr), RowAction::OnlyClient(entry.addr));
                    item(ui, format!("Hide {}", entry.addr), RowAction::HideClient(entry.addr));
                    ui.separator();
                    item(ui, "Reset filters".into(), RowAction::ResetFilter);
                });
            });
        });

    match action {
        Some(RowAction::Select(index)) => capture.select(index),
        Some(RowAction::Rule(name, rule)) => capture.filter.set_rule(name, rule),
        Some(RowAction::OnlyClient(addr)) => capture.filter.set_client_rule(addr, Rule::Only),
        Some(RowAction::HideClient(addr)) => capture.filter.set_client_rule(addr, Rule::Hide),
        Some(RowAction::ResetFilter) => capture.filter.reset(),
        None => {}
    }
}

fn log_view(ui: &mut Ui, capture: &Capture) {
    ScrollArea::vertical().auto_shrink(false).stick_to_bottom(true).show(ui, |ui| {
        if capture.events.is_empty() {
            ui.label(RichText::new("Connections, disconnects and errors show up here.").color(theme::TEXT_DIM));
        }
        for event in &capture.events {
            ui.label(RichText::new(event).monospace());
        }
    });
}

pub fn status_bar(ui: &mut Ui, capture: &Capture, running: bool) {
    Panel::bottom("status").frame(theme::panel_frame(theme::SURFACE).inner_margin(egui::Margin::symmetric(10, 4))).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (dot, text) = match (running, capture.motd.is_empty()) {
                (false, _) => (theme::TEXT_DIM, "Stopped".to_string()),
                (true, true) => (theme::DANGER, "Target not responding".to_string()),
                (true, false) => (theme::CLIENT, capture.motd.clone()),
            };
            ui.label(RichText::new(egui_phosphor::fill::CIRCLE).small().color(dot));
            ui.label(RichText::new(text).small().color(theme::TEXT_DIM));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if capture.paused {
                    ui.label(RichText::new("PAUSED").small().strong().color(theme::DANGER));
                }
                ui.label(RichText::new(format!("{} packets", capture.entries.len())).small().color(theme::TEXT_DIM));
            });
        });
    });
}

fn format_size(bytes: usize) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KB", bytes as f32 / 1024.),
        _ => format!("{:.1} MB", bytes as f32 / 1_048_576.),
    }
}
