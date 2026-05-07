pub mod network;
pub mod ui;

use crate::network::event::NetworkEvent;
use crate::network::source::Source;
use crate::network::Network;
use bedrock::network::connection::Connection;
use bedrock::protocol::{DynPacket, Packets, V975};
use chrono::{DateTime, Local};
use eframe::{run_native, App, NativeOptions, Result};
use egui::{CentralPanel, CollapsingHeader, Color32, Ui};
use std::collections::BTreeMap;
use std::fmt::Debug;

pub type BedrockProtocol = V975;
pub type BedrockConnection = Connection<BedrockProtocol>;

#[tokio::main]
async fn main() -> Result<()> {
    run_native(
        "Gateway",
        NativeOptions::default(),
        Box::new(|cc| { 
            let mut fonts = egui::FontDefinitions::default();
            
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Fill);
            
            cc.egui_ctx.set_fonts(fonts);
            
            Ok(Box::<GatewayApp>::default()) 
        })
    )
}

struct GatewayApp {
    state: AppState,
}

#[derive(Debug)]
enum PacketSource {
    Server,
    Client,
}

#[derive(Debug)]
struct PacketEntry {
    timestamp: DateTime<Local>,
    source: Source,
    packet: Box<dyn DynPacket>
}

enum AppState {
    Setup {
        proxy_addr: String,
        proxy_addr_valid: bool,
        server_addr: String,
        server_addr_valid: bool,
    },
    Running {
        network: Network,
        packets: BTreeMap<String, Vec<PacketEntry>>
    },
}

impl Default for GatewayApp {
    fn default() -> Self {
        Self {
            state: AppState::Setup {
                proxy_addr: "0.0.0.0:19132".into(),
                proxy_addr_valid: true,
                server_addr: "127.0.0.1:19133".into(),
                server_addr_valid: true,
            }
        }
    }
}

impl App for GatewayApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let mut next_state = None;
        
        match &mut self.state {
            AppState::Running { network, packets } => {
                while let Ok(ev) = network.ev_rx.try_recv() {
                    match ev {
                        NetworkEvent::Packet {
                            packet,
                            source,
                            ..
                        } => {
                            let packet = packet.into_inner();
                            let full = packet.name();

                            let no_generics = full.split('<').next().unwrap_or(full);
                            let name = no_generics.rsplit("::").next().unwrap_or(no_generics);

                            packets.entry(name.into())
                                .or_default()
                                .push(PacketEntry {
                                    timestamp: Local::now(),
                                    source,
                                    packet,
                                });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    
        ui::toolbar::toolbar(ui, &mut self.state);
    
        CentralPanel::default().show_inside(ui, |ui| {
            match &self.state {
                AppState::Running { packets, .. } => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (packet_name, list) in packets.iter().rev() {
                            let header = format!("{} ({})", packet_name, list.len());

                            CollapsingHeader::new(header)
                                .default_open(false)
                                .show(ui, |ui| {
                                    for packet in list {
                                        ui.horizontal(|ui| {
                                            let ts = packet.timestamp.format("%H:%M:%S%.3f").to_string();
                                            ui.label(format!("[{}]", ts));

                                            match packet.source {
                                                Source::Server => {
                                                    ui.colored_label(Color32::LIGHT_BLUE, "SERVER");
                                                }
                                                Source::Client => {
                                                    ui.colored_label(Color32::LIGHT_GREEN, "CLIENT");
                                                }
                                            }

                                            ui.label(format!("{:?}", packet.packet));
                                        });
                                    }
                                });
                        }
                    });
                }
                _ => {}
            }
        });
    
        if let Some(state) = next_state {
            self.state = state;
        }
    }
}