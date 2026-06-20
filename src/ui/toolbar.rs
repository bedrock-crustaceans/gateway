use crate::network::Network;
use crate::AppState;
use eframe::epaint::Color32;
use egui::{vec2, Button, Panel, RichText, TextEdit, Ui};
use egui_phosphor::variants::fill;
use std::collections::BTreeMap;

pub fn toolbar(ui: &mut Ui, state: &mut AppState) {
    Panel::top("toolbar").show_inside(ui, |ui| {
        ui.horizontal(|ui| {
            match state {
                AppState::Setup { proxy_addr: client_addr, proxy_addr_valid: client_addr_valid, server_addr, server_addr_valid } => {
                    let button = ui.scope(|ui| {
                        ui.style_mut().spacing.button_padding = vec2(4., 4.);
                        
                        ui.add(Button::new(
                            RichText::new(fill::PLAY)
                                .color(Color32::from_rgb(130, 200, 150))
                        ).frame_when_inactive(false))
                    }).inner;

                    if button.clicked() {
                        match (client_addr.parse(), server_addr.parse()) {
                            (Ok(c), Ok(s)) => {
                                *state = AppState::Running {
                                    network: Network::new(c, s),
                                    packets: BTreeMap::new(),
                                    pong_msg: "".to_string()
                                };
                                return;
                            }
                            (c, s) => {
                                *client_addr_valid = c.is_ok();
                                *server_addr_valid = s.is_ok();
                            },
                        }
                    }

                    ui.add(
                        match client_addr_valid {
                            true => TextEdit::singleline(client_addr).prefix("Proxy"),
                            false => TextEdit::singleline(client_addr).prefix("Proxy").background_color(Color32::RED)
                        },
                    );

                    ui.add(
                        match server_addr_valid {
                            true => TextEdit::singleline(server_addr).prefix("Server"),
                            false => TextEdit::singleline(server_addr).prefix("Server").background_color(Color32::RED)
                        },
                    );
                }

                AppState::Running { network, .. } => {
                    let button = ui.scope(|ui| {
                        ui.style_mut().spacing.button_padding = vec2(4., 4.);
                        
                        ui.add(Button::new(
                            RichText::new(fill::STOP)
                                .color(Color32::from_rgb(220, 140, 140))
                        ).frame_when_inactive(false))
                    }).inner;

                    if button.clicked() {
                        network.close();

                        *state = AppState::Setup {
                            proxy_addr: network.rx_addr.to_string(),
                            proxy_addr_valid: true,
                            server_addr: network.tx_addr.to_string(),
                            server_addr_valid: true,
                        };
                        return;
                    }

                    let mut c = network.rx_addr.to_string();
                    ui.add_enabled(
                        false,
                        TextEdit::singleline(&mut c).prefix("Proxy")
                    );

                    let mut s = network.tx_addr.to_string();
                    ui.add_enabled(
                        false,
                        TextEdit::singleline(&mut s).prefix("Server")
                    );
                }
            }
        });
    });
}