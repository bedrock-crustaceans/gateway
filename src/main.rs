pub mod network;
pub mod ui;

use crate::network::event::NetworkEvent;
use crate::network::{Network, Transport};
use crate::ui::capture::Capture;
use bedrock::network::tokio::Connection;
use bedrock::protocol::V2193;
use eframe::{get_value, run_native, set_value, App, NativeOptions, Result, Storage};
use egui::{CentralPanel, Ui};
use std::time::Duration;

pub type BedrockProtocol = V2193;
pub type BedrockConnection = Connection<BedrockProtocol>;

#[tokio::main]
async fn main() -> Result<()> {
    run_native(
        "Gateway",
        NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1440., 860.]).with_min_inner_size([900., 500.]),
            ..Default::default()
        },
        Box::new(|cc| {
            let mut fonts = egui::FontDefinitions::default();

            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
            egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Fill);

            cc.egui_ctx.set_fonts(fonts);
            ui::theme::apply(&cc.egui_ctx);

            Ok(Box::new(GatewayApp::load(cc.storage)))
        }),
    )
}

pub struct GatewayApp {
    pub state: AppState,
    pub capture: Capture,
}

pub enum AppState {
    Setup {
        transport: Transport,
        proxy_addr: String,
        proxy_addr_valid: bool,
        server_addr: String,
        server_addr_valid: bool,
    },
    Running {
        network: Network,
    },
}

const TRANSPORT_KEY: &str = "transport";
const PROXY_ADDR_KEY: &str = "proxy_addr";
const SERVER_ADDR_KEY: &str = "server_addr";
const FILTER_KEY: &str = "filter";
const INSPECTOR_TAB_KEY: &str = "inspector_tab";
const FOLLOW_KEY: &str = "follow";

impl GatewayApp {
    fn load(storage: Option<&dyn Storage>) -> Self {
        let get = |key| storage.and_then(|s| get_value::<String>(s, key));
        let mut capture = Capture::default();

        if let Some(storage) = storage {
            capture.filter = get_value(storage, FILTER_KEY).unwrap_or_default();
            capture.inspector_tab = get_value(storage, INSPECTOR_TAB_KEY).unwrap_or_default();
            capture.follow = get_value(storage, FOLLOW_KEY).unwrap_or(true);
        }

        Self {
            state: AppState::Setup {
                transport: storage.and_then(|s| get_value(s, TRANSPORT_KEY)).unwrap_or_default(),
                proxy_addr: get(PROXY_ADDR_KEY).unwrap_or_else(|| "0.0.0.0:19132".into()),
                proxy_addr_valid: true,
                server_addr: get(SERVER_ADDR_KEY).unwrap_or_else(|| "127.0.0.1:19133".into()),
                server_addr_valid: true,
            },
            capture,
        }
    }
}

impl App for GatewayApp {
    fn save(&mut self, storage: &mut dyn Storage) {
        let (transport, proxy_addr, server_addr) = match &self.state {
            AppState::Setup { transport, proxy_addr, server_addr, .. } => (*transport, proxy_addr.clone(), server_addr.clone()),
            AppState::Running { network } => (network.transport, network.rx_addr.to_string(), network.tx_addr.to_string()),
        };
        set_value(storage, TRANSPORT_KEY, &transport);
        set_value(storage, PROXY_ADDR_KEY, &proxy_addr);
        set_value(storage, SERVER_ADDR_KEY, &server_addr);
        set_value(storage, FILTER_KEY, &self.capture.filter);
        set_value(storage, INSPECTOR_TAB_KEY, &self.capture.inspector_tab);
        set_value(storage, FOLLOW_KEY, &self.capture.follow);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        if let AppState::Running { network } = &mut self.state {
            while let Ok(ev) = network.ev_rx.try_recv() {
                match ev {
                    NetworkEvent::Packet { packet, frame, source, addr } => {
                        self.capture.push(packet, frame, source, addr);
                    }
                    NetworkEvent::Pong(msg) => self.capture.set_motd(&msg),
                    NetworkEvent::Connected(addr) => self.capture.log(format!("{addr} connected")),
                    NetworkEvent::Disconnected(addr, reason) => {
                        self.capture.log(format!("{addr} disconnected: {reason}"))
                    }
                    NetworkEvent::Error(err) => self.capture.log(err),
                }
            }

            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }

        ui::toolbar::toolbar(ui, self);
        let running = matches!(self.state, AppState::Running { .. });
        ui::capture::status_bar(ui, &self.capture, running);
        ui::inspector::inspector(ui, &mut self.capture);

        CentralPanel::default().frame(ui::theme::panel_frame(ui::theme::BG)).show(ui, |ui| {
            ui::capture::main_view(ui, &mut self.capture);
        });
    }
}
