use eframe::{run_native, NativeOptions, Result};
use egui::{CentralPanel, Color32};
use std::net::SocketAddr;

fn main() -> Result<()> {
    run_native(
        "Gateway",
        NativeOptions::default(),
        Box::new(|_| Ok(Box::<GatewayApp>::default()))
    )
}

struct GatewayApp {
    state: AppState
}

#[derive(Clone, Debug)]
enum AppState {
    Setup {
        client_addr: String,
        server_addr: String,
        error: Option<String>,
    },
    Running {
        client_addr: SocketAddr,
        server_addr: SocketAddr,
    },
}

impl Default for GatewayApp {
    fn default() -> Self {
        Self {
            state: AppState::Setup {
                client_addr: "0.0.0.0:19132".into(),
                server_addr: "127.0.0.1:19133".into(),
                error: None,
            }
        }
    }
}

impl eframe::App for GatewayApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let mut next_state = None;
        
        match &mut self.state {
            AppState::Setup { client_addr, server_addr, error } => {
                CentralPanel::default().show_inside(ui, |ui| {
                    ui.heading("Gateway Configuration");

                    ui.label("Client Address:");
                    ui.text_edit_singleline(client_addr);

                    ui.label("Server Address:");
                    ui.text_edit_singleline(server_addr);

                    if let Some(err) = &error {
                        ui.colored_label(Color32::RED, err);
                    }

                    ui.add_space(10.0);

                    if ui.button("Start").clicked() {
                        match (client_addr.parse::<SocketAddr>(), server_addr.parse::<SocketAddr>()) {
                            (Ok(c), Ok(s)) => {
                                next_state = Some(AppState::Running {
                                    client_addr: c,
                                    server_addr: s,
                                });
                            }
                            _ => *error = Some("Invalid Address".into()),
                        }
                    }
                });
            }

            AppState::Running { client_addr, server_addr } => {
                CentralPanel::default().show_inside(ui, |ui| {
                    ui.heading("Gateway Running");
                    
                    ui.label(format!("Client Address: {}", client_addr));
                    ui.label(format!("Server Address: {}", server_addr));

                    if ui.button("Stop").clicked() {
                        next_state = Some(AppState::Setup {
                            client_addr: "0.0.0.0:19132".into(),
                            server_addr: "127.0.0.1:19133".into(),
                            error: None,
                        });
                    }
                });
            }
        };
        
        if let Some(state) = next_state {
            self.state = state;
        }
    }
}