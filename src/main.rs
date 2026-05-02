use bedrock::protocol::v898::packets::TextPacket;
use bedrock::protocol::v924::enums::TextPacketType;
use bedrock::protocol::V944;
use eframe::{run_native, App, NativeOptions, Result};
use egui::*;
use egui_material_icons::icons::{ICON_PLAY_ARROW, ICON_STOP};
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::net::SocketAddr;
use std::time::Instant;

fn main() -> Result<()> {
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
    state: AppState
}

trait PacketData: Any + Debug + Send {}

impl<T: Any + Debug + Send> PacketData for T {}

#[derive(Debug)]
enum PacketSource {
    Server,
    Client,
}

#[derive(Debug)]
struct PacketEntry {
    timestamp: Instant,
    source: PacketSource,
    packet: Box<dyn PacketData>
}

fn fake_packets() -> BTreeMap<String, Vec<PacketEntry>> {
    use PacketSource::*;

    let mut map: BTreeMap<String, Vec<PacketEntry>> = BTreeMap::new();

    fn insert<T: PacketData> (map: &mut BTreeMap<String, Vec<PacketEntry>>, packet: Box<T>, source: PacketSource) {
        let full = std::any::type_name::<T>();
        
        let no_generics = full.split('<').next().unwrap_or(full);
        let name = no_generics.rsplit("::").next().unwrap_or(no_generics);

        map.entry(name.into())
            .or_default()
            .push(PacketEntry {
                timestamp: Instant::now(),
                source,
                packet,
            });
    }

    insert(
        &mut map,
        Box::new(TextPacket::<V944> {
            localize: false,
            message_type: TextPacketType::Chat { 
                player_name: "".to_string(), 
                message: "".to_string() 
            },
            sender_xuid: "".to_string(),
            platform_id: "".to_string(),
            filtered_message: None,
        }),
        Client,
    );

    map
}

#[derive(Debug)]
enum AppState {
    Setup {
        client_addr: String,
        client_addr_valid: bool,
        server_addr: String,
        server_addr_valid: bool,
    },
    Running {
        client_addr: SocketAddr,
        server_addr: SocketAddr,
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
                                        client_addr: c,
                                        server_addr: s,
                                        packets: fake_packets()
                                    });
                                }
                                (c, s) => {
                                    *client_addr_valid = c.is_ok();
                                    *server_addr_valid = s.is_ok();
                                },
                            }
                        }
                    }
    
                    AppState::Running { client_addr, server_addr, .. } => {
                        ui.label("Client:");
                        
                        let mut c = client_addr.to_string();
                        ui.add_enabled(
                            false,
                            TextEdit::singleline(&mut c)
                        );

                        ui.label("Server:");
                        
                        let mut s = server_addr.to_string();
                        ui.add_enabled(
                            false,
                            TextEdit::singleline(&mut s)
                        );

                        let button = Button::new(
                            RichText::new(ICON_STOP)
                                .color(Color32::from_rgb(220, 140, 140))
                        );
    
                        if ui.add(button).clicked() {
                            next_state = Some(AppState::Setup {
                                client_addr: client_addr.to_string(),
                                client_addr_valid: true,
                                server_addr: server_addr.to_string(),
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
                                            let ts = packet.timestamp.elapsed().as_secs_f32();
                                            ui.label(format!("[{:.2}s ago]", ts));

                                            match packet.source {
                                                PacketSource::Server => {
                                                    ui.colored_label(Color32::LIGHT_BLUE, "SERVER");
                                                }
                                                PacketSource::Client => {
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