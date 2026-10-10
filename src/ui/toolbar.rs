use crate::network::{Network, Transport};
use crate::ui::filter;
use crate::ui::theme;
use crate::{AppState, GatewayApp};
use egui::{Align, Button, Color32, Layout, Panel, Response, RichText, Stroke, TextEdit, Ui};
use egui_phosphor::{fill, regular};

fn primary_button(ui: &mut Ui, icon: &str, text: &str, color: Color32) -> Response {
    ui.add(
        Button::new(RichText::new(format!("{icon}  {text}")).color(color).strong())
            .fill(color.gamma_multiply(0.12))
            .stroke(Stroke::new(1., color.gamma_multiply(0.5))),
    )
}

fn icon_button(ui: &mut Ui, icon: &str, hint: &str, active: bool) -> Response {
    let color = if active { theme::ACCENT } else { theme::TEXT_DIM };
    ui.add(Button::new(RichText::new(icon).size(16.).color(color)).frame_when_inactive(active)).on_hover_text(hint)
}

fn addr_field(ui: &mut Ui, label: &str, value: &mut String, valid: bool, enabled: bool) {
    ui.label(RichText::new(label).small().color(theme::TEXT_DIM));
    let mut edit = TextEdit::singleline(value).desired_width(150.).font(egui::TextStyle::Monospace);
    if !valid {
        edit = edit.background_color(theme::DANGER.gamma_multiply(0.25));
    }
    ui.add_enabled(enabled, edit);
}

pub fn toolbar(ui: &mut Ui, app: &mut GatewayApp) {
    Panel::top("toolbar").frame(theme::panel_frame(theme::SURFACE)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Gateway").heading().strong());
            ui.add_space(8.);

            match &mut app.state {
                AppState::Setup { transport, proxy_addr, proxy_addr_valid, server_addr, server_addr_valid } => {
                    egui::ComboBox::from_id_salt("transport").selected_text(transport.label()).show_ui(ui, |ui| {
                        for option in Transport::ALL {
                            ui.selectable_value(transport, option, option.label());
                        }
                    });
                    addr_field(ui, "Proxy", proxy_addr, *proxy_addr_valid, true);
                    addr_field(ui, "Target", server_addr, *server_addr_valid, true);

                    if primary_button(ui, fill::PLAY, "Start", theme::CLIENT).clicked() {
                        match (proxy_addr.parse(), server_addr.parse()) {
                            (Ok(proxy), Ok(server)) => {
                                app.capture.clear();
                                app.capture.motd.clear();
                                app.state = AppState::Running { network: Network::new(*transport, proxy, server) };
                                return;
                            }
                            (proxy, server) => {
                                *proxy_addr_valid = proxy.is_ok();
                                *server_addr_valid = server.is_ok();
                            }
                        }
                    }
                }

                AppState::Running { network } => {
                    ui.label(RichText::new(network.transport.label()).small().color(theme::TEXT_DIM));
                    addr_field(ui, "Proxy", &mut network.rx_addr.to_string(), true, false);
                    addr_field(ui, "Target", &mut network.tx_addr.to_string(), true, false);

                    if primary_button(ui, fill::STOP, "Stop", theme::DANGER).clicked() {
                        network.close();
                        app.state = AppState::Setup {
                            transport: network.transport,
                            proxy_addr: network.rx_addr.to_string(),
                            proxy_addr_valid: true,
                            server_addr: network.tx_addr.to_string(),
                            server_addr_valid: true,
                        };
                        return;
                    }
                }
            }

            let capture = &mut app.capture;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if icon_button(ui, regular::TRASH, "Clear capture", false).clicked() {
                    capture.clear();
                }
                let (icon, hint) = if capture.paused { (fill::PLAY_CIRCLE, "Resume capture") } else { (fill::PAUSE_CIRCLE, "Pause capture") };
                if icon_button(ui, icon, hint, capture.paused).clicked() {
                    capture.paused = !capture.paused;
                }
                if icon_button(ui, regular::ARROW_LINE_DOWN, "Follow newest packets", capture.follow).clicked() {
                    capture.follow = !capture.follow;
                }

                ui.separator();
                let filter = &mut capture.filter;

                let rules = filter.active_rules();
                let (icon, color) = if rules == 0 { (regular::FUNNEL, theme::TEXT_DIM) } else { (fill::FUNNEL, theme::ACCENT) };
                let text = if rules == 0 { format!("{icon}  Filters") } else { format!("{icon}  Filters  {rules}") };
                let button = ui.add(Button::new(RichText::new(text).color(color)).stroke(Stroke::new(1., if rules == 0 { theme::BORDER } else { color.gamma_multiply(0.5) })));
                filter::popover(&button, filter, &capture.entries);
                filter::direction_dropdown(ui, filter);

                let errors = filter.query().errors.join("\n");
                let mut edit = TextEdit::singleline(&mut filter.text)
                    .hint_text(format!("{}  text -move dir:s2c id:9 size:>100", regular::MAGNIFYING_GLASS))
                    .desired_width(260.)
                    .font(egui::TextStyle::Monospace);
                if !errors.is_empty() {
                    edit = edit.background_color(theme::DANGER.gamma_multiply(0.2));
                }
                let response = ui.add(edit);
                response.on_hover_text(if errors.is_empty() { filter::QUERY_HELP.to_string() } else { errors });
            });
        });
    });
}
