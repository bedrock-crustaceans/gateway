pub mod network;

use crate::network::event::NetworkEvent;
use crate::network::Network;
use bedrock::network::connection::Connection;
use bedrock::protocol::{DynPacket, Packets, V944};
use chrono::{DateTime, Local};
use eframe::{run_native, App, NativeOptions, Result};
use egui::{Button, CentralPanel, CollapsingHeader, Color32, Panel, RichText, TextEdit, Ui};
use egui_material_icons::icons::{ICON_PLAY_ARROW, ICON_STOP};
use std::collections::BTreeMap;
use std::fmt::Debug;
use crate::network::direction::Direction;

pub type BedrockProtocol = V944;
pub type BedrockConnection = Connection<BedrockProtocol>;

#[tokio::main]
async fn main() -> Result<()> {
    run_native(
        "Gateway",
        NativeOptions::default(),
        Box::new(|cc| { 
            egui_material_icons::initialize(&cc.egui_ctx);
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
    direction: Direction,
    packet: Box<dyn DynPacket>
}

enum AppState {
    Setup {
        client_addr: String,
        client_addr_valid: bool,
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
                client_addr: "0.0.0.0:19132".into(),
                client_addr_valid: true,
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
                            direction,
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
                                    direction,
                                    packet,
                                });
                        },
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    
        Panel::top("top_bar").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                match &mut self.state {
                    AppState::Setup { client_addr, client_addr_valid, server_addr, server_addr_valid } => {
                        ui.label("Client:");
                        ui.add(
                            match client_addr_valid {
                                true => TextEdit::singleline(client_addr),
                                false => TextEdit::singleline(client_addr).background_color(Color32::RED)
                            },
                        );
    
                        ui.label("Server:");
                        ui.add(
                            match server_addr_valid {
                                true => TextEdit::singleline(server_addr),
                                false => TextEdit::singleline(server_addr).background_color(Color32::RED)
                            },
                        );

                        let button = Button::new(
                            RichText::new(ICON_PLAY_ARROW)
                                .color(Color32::from_rgb(130, 200, 150))
                        );
    
                        if ui.add(button).clicked() {
                            match (client_addr.parse(), server_addr.parse()) {
                                (Ok(c), Ok(s)) => {
                                    next_state = Some(AppState::Running {
                                        network: Network::new(c, s),
                                        packets: BTreeMap::new()
                                    });
                                }
                                (c, s) => {
                                    *client_addr_valid = c.is_ok();
                                    *server_addr_valid = s.is_ok();
                                },
                            }
                        }
                    }
    
                    AppState::Running { network, .. } => {
                        ui.label("Client:");
                        
                        let mut c = network.rx_addr.to_string();
                        ui.add_enabled(
                            false,
                            TextEdit::singleline(&mut c)
                        );

                        ui.label("Server:");
                        
                        let mut s = network.tx_addr.to_string();
                        ui.add_enabled(
                            false,
                            TextEdit::singleline(&mut s)
                        );

                        let button = Button::new(
                            RichText::new(ICON_STOP)
                                .color(Color32::from_rgb(220, 140, 140))
                        );
    
                        if ui.add(button).clicked() {
                            network.close();
                            
                            next_state = Some(AppState::Setup {
                                client_addr: network.rx_addr.to_string(),
                                client_addr_valid: true,
                                server_addr: network.tx_addr.to_string(),
                                server_addr_valid: true,
                            });
                        }
                    }
                }
            });
        });
    
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

                                            match packet.direction {
                                                Direction::Downstream => {
                                                    ui.colored_label(Color32::LIGHT_BLUE, "DOWN");
                                                }
                                                Direction::Upstream => {
                                                    ui.colored_label(Color32::LIGHT_GREEN, "UP");
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