use crate::network::source::Source;
use crate::ui::capture::PacketEntry;
use crate::ui::theme;
use egui::{Align, Button, Color32, CornerRadius, Frame, Layout, Popup, PopupCloseBehavior, Response, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, vec2};
use egui_phosphor::regular;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Rule {
    Show,
    Only,
    Hide,
}

impl Rule {
    fn icon(self) -> &'static str {
        match self {
            Rule::Show => regular::EYE,
            Rule::Only => regular::CROSSHAIR,
            Rule::Hide => regular::EYE_SLASH,
        }
    }

    fn color(self) -> Color32 {
        match self {
            Rule::Show => theme::TEXT_DIM,
            Rule::Only => theme::CLIENT,
            Rule::Hide => theme::DANGER,
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Rule::Show => "Show",
            Rule::Only => "Only show these",
            Rule::Hide => "Hide",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum SizeOp {
    Less,
    Greater,
    Equal,
}

#[derive(Default, Debug, PartialEq)]
pub struct Query {
    include: Vec<String>,
    exclude: Vec<String>,
    ids: Vec<u16>,
    source: Option<Source>,
    from: Vec<String>,
    sizes: Vec<(SizeOp, usize)>,
    pub errors: Vec<String>,
}

impl Query {
    pub fn parse(text: &str) -> Self {
        let mut query = Query::default();

        for term in text.split([' ', ',']).map(str::trim).filter(|t| !t.is_empty()) {
            let term = term.to_lowercase();

            if let Some(name) = term.strip_prefix('-').or_else(|| term.strip_prefix('!')) {
                if !name.is_empty() {
                    query.exclude.push(name.to_string());
                }
                continue;
            }

            let Some((key, value)) = term.split_once(':') else {
                query.include.push(term);
                continue;
            };

            match key {
                "id" => match value.parse() {
                    Ok(id) => query.ids.push(id),
                    Err(_) => query.errors.push(format!("id:{value} is not a number")),
                },
                "dir" => match value {
                    "c2s" | "client" | "c" => query.source = Some(Source::Client),
                    "s2c" | "server" | "s" => query.source = Some(Source::Server),
                    _ => query.errors.push(format!("dir:{value} should be c2s or s2c")),
                },
                "from" | "addr" => query.from.push(value.to_string()),
                "size" => {
                    let (op, number) = match value.as_bytes().first() {
                        Some(b'<') => (SizeOp::Less, &value[1..]),
                        Some(b'>') => (SizeOp::Greater, &value[1..]),
                        Some(b'=') => (SizeOp::Equal, &value[1..]),
                        _ => (SizeOp::Equal, value),
                    };
                    match number.parse() {
                        Ok(n) => query.sizes.push((op, n)),
                        Err(_) => query.errors.push(format!("size:{value} should look like size:>100")),
                    }
                }
                _ => query.include.push(term),
            }
        }

        query
    }

    pub fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty() && self.ids.is_empty() && self.source.is_none() && self.from.is_empty() && self.sizes.is_empty()
    }

    fn matches(&self, packet: &PacketInfo) -> bool {
        let name = packet.name.to_lowercase();
        let addr = packet.addr.to_string();

        (self.include.is_empty() || self.include.iter().any(|t| name.contains(t.as_str())))
            && !self.exclude.iter().any(|t| name.contains(t.as_str()))
            && (self.ids.is_empty() || self.ids.contains(&packet.id))
            && self.source.is_none_or(|s| s == packet.source)
            && (self.from.is_empty() || self.from.iter().any(|f| addr.contains(f.as_str())))
            && self.sizes.iter().all(|&(op, n)| match op {
                SizeOp::Less => packet.size < n,
                SizeOp::Greater => packet.size > n,
                SizeOp::Equal => packet.size == n,
            })
    }
}

pub struct PacketInfo<'a> {
    pub name: &'a str,
    pub id: u16,
    pub source: Source,
    pub addr: SocketAddr,
    pub size: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Filter {
    pub text: String,
    pub show_client: bool,
    pub show_server: bool,
    pub only: BTreeSet<String>,
    pub hidden: BTreeSet<String>,
    #[serde(skip)]
    pub only_clients: BTreeSet<SocketAddr>,
    #[serde(skip)]
    pub hidden_clients: BTreeSet<SocketAddr>,
    #[serde(skip)]
    packet_search: String,
    #[serde(skip)]
    query: Query,
    #[serde(skip)]
    parsed_text: Option<String>,
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            text: String::new(),
            show_client: true,
            show_server: true,
            only: BTreeSet::new(),
            hidden: BTreeSet::new(),
            only_clients: BTreeSet::new(),
            hidden_clients: BTreeSet::new(),
            packet_search: String::new(),
            query: Query::default(),
            parsed_text: None,
        }
    }
}

impl Filter {
    pub fn query(&mut self) -> &Query {
        if self.parsed_text.as_deref() != Some(self.text.as_str()) {
            self.query = Query::parse(&self.text);
            self.parsed_text = Some(self.text.clone());
        }
        &self.query
    }

    pub fn rule(&self, name: &str) -> Rule {
        if self.only.contains(name) {
            Rule::Only
        } else if self.hidden.contains(name) {
            Rule::Hide
        } else {
            Rule::Show
        }
    }

    pub fn set_rule(&mut self, name: &str, rule: Rule) {
        self.only.remove(name);
        self.hidden.remove(name);
        match rule {
            Rule::Show => {}
            Rule::Only => {
                self.only.insert(name.to_string());
            }
            Rule::Hide => {
                self.hidden.insert(name.to_string());
            }
        }
    }

    pub fn client_rule(&self, addr: SocketAddr) -> Rule {
        if self.only_clients.contains(&addr) {
            Rule::Only
        } else if self.hidden_clients.contains(&addr) {
            Rule::Hide
        } else {
            Rule::Show
        }
    }

    pub fn set_client_rule(&mut self, addr: SocketAddr, rule: Rule) {
        self.only_clients.remove(&addr);
        self.hidden_clients.remove(&addr);
        match rule {
            Rule::Show => {}
            Rule::Only => {
                self.only_clients.insert(addr);
            }
            Rule::Hide => {
                self.hidden_clients.insert(addr);
            }
        }
    }

    pub fn active_rules(&self) -> usize {
        self.only.len() + self.hidden.len() + self.only_clients.len() + self.hidden_clients.len()
    }

    pub fn reset(&mut self) {
        *self = Self {
            text: std::mem::take(&mut self.text),
            ..Self::default()
        };
    }

    pub fn matches(&mut self, packet: &PacketInfo) -> bool {
        let source_shown = match packet.source {
            Source::Client => self.show_client,
            Source::Server => self.show_server,
        };

        source_shown
            && !self.hidden_clients.contains(&packet.addr)
            && (self.only_clients.is_empty() || self.only_clients.contains(&packet.addr))
            && !self.hidden.contains(packet.name)
            && (self.only.is_empty() || self.only.contains(packet.name))
            && self.query().matches(packet)
    }
}

pub const QUERY_HELP: &str = "name words      show packets matching any word
-name or !name  hide matching packets
id:9            packet id
dir:c2s / s2c   direction
from:1.2.3.4    client address contains
size:>1000      size in bytes (<, >, =)";

fn rule_toggle(ui: &mut Ui, current: Rule) -> Option<Rule> {
    let mut picked = None;
    Frame::new().fill(theme::BG).stroke(Stroke::new(1., theme::BORDER)).corner_radius(5).inner_margin(2).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.;
        ui.spacing_mut().button_padding = vec2(6., 1.);
        for rule in [Rule::Show, Rule::Only, Rule::Hide] {
            let active = current == rule;
            let (color, fill) = match (active, rule) {
                (false, _) => (theme::TEXT_DIM.gamma_multiply(0.7), Color32::TRANSPARENT),
                (true, Rule::Show) => (theme::TEXT, theme::SURFACE_RAISED),
                (true, _) => (rule.color(), rule.color().gamma_multiply(0.2)),
            };
            let button = Button::new(RichText::new(rule.icon()).color(color)).fill(fill).stroke(Stroke::NONE).corner_radius(4);
            if ui.add(button).on_hover_text(rule.hint()).clicked() && !active {
                picked = Some(rule);
            }
        }
    });
    picked
}

fn chip(ui: &mut Ui, text: String, color: Color32) -> bool {
    let button = Button::new(RichText::new(format!("{text}  {}", regular::X)).small().color(color))
        .fill(color.gamma_multiply(0.15))
        .stroke(Stroke::new(1., color.gamma_multiply(0.4)))
        .corner_radius(CornerRadius::same(10));
    ui.add(button).on_hover_text("Remove").clicked()
}

fn section(ui: &mut Ui, title: &str) {
    ui.add_space(10.);
    ui.label(RichText::new(title).small().strong().color(theme::TEXT_DIM));
    ui.add_space(2.);
}

fn rule_rows<K: Copy>(ui: &mut Ui, rows: &[(K, String, usize, Rule)], monospace: bool) -> Option<(K, Rule)> {
    let mut picked = None;
    for (key, label, count, rule) in rows {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 28.), Sense::hover());
        if *rule != Rule::Show {
            ui.painter().rect_filled(rect, 4, rule.color().gamma_multiply(0.07));
        }
        let mut row = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(6., 0.))).layout(Layout::right_to_left(Align::Center)));
        if let Some(new) = rule_toggle(&mut row, *rule) {
            picked = Some((*key, new));
        }
        row.label(RichText::new(count.to_string()).small().color(theme::TEXT_DIM));
        row.with_layout(Layout::left_to_right(Align::Center), |ui| {
            let mut text = RichText::new(label).color(if *rule == Rule::Hide { theme::TEXT_DIM } else { theme::TEXT });
            if monospace {
                text = text.monospace();
            }
            ui.add(egui::Label::new(text).truncate().selectable(false));
        });
    }
    picked
}

#[derive(Copy, Clone, PartialEq)]
enum Direction {
    Both,
    Client,
    Server,
}

impl Direction {
    fn label(self) -> String {
        match self {
            Direction::Both => "Both directions".to_string(),
            Direction::Client => format!("{} only", theme::direction(Source::Client).0),
            Direction::Server => format!("{} only", theme::direction(Source::Server).0),
        }
    }
}

pub fn direction_dropdown(ui: &mut Ui, filter: &mut Filter) {
    let mut direction = match (filter.show_client, filter.show_server) {
        (true, false) => Direction::Client,
        (false, true) => Direction::Server,
        _ => Direction::Both,
    };
    let before = direction;

    egui::ComboBox::from_id_salt("direction")
        .selected_text(direction.label())
        .width(130.)
        .show_ui(ui, |ui| {
            for option in [Direction::Both, Direction::Client, Direction::Server] {
                ui.selectable_value(&mut direction, option, option.label());
            }
        });

    if direction != before || !(filter.show_client || filter.show_server) {
        (filter.show_client, filter.show_server) = match direction {
            Direction::Both => (true, true),
            Direction::Client => (true, false),
            Direction::Server => (false, true),
        };
    }
}

pub fn popover(button: &Response, filter: &mut Filter, entries: &[PacketEntry]) {
    Popup::from_toggle_button_response(button)
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .width(380.)
        .show(|ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Filters").strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let reset = Button::new(RichText::new("Reset").small().color(theme::ACCENT)).frame(false);
                    if ui.add_enabled(filter.active_rules() > 0, reset).clicked() {
                        filter.reset();
                    }
                });
            });

            if filter.active_rules() > 0 {
                ui.add_space(6.);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(4., 4.);
                    for name in filter.only.clone() {
                        if chip(ui, format!("only {name}"), theme::CLIENT) {
                            filter.set_rule(&name, Rule::Show);
                        }
                    }
                    for name in filter.hidden.clone() {
                        if chip(ui, format!("hide {name}"), theme::DANGER) {
                            filter.set_rule(&name, Rule::Show);
                        }
                    }
                    for addr in filter.only_clients.clone() {
                        if chip(ui, format!("only {addr}"), theme::CLIENT) {
                            filter.set_client_rule(addr, Rule::Show);
                        }
                    }
                    for addr in filter.hidden_clients.clone() {
                        if chip(ui, format!("hide {addr}"), theme::DANGER) {
                            filter.set_client_rule(addr, Rule::Show);
                        }
                    }
                });
            }

            let mut packets: BTreeMap<&str, usize> = BTreeMap::new();
            let mut clients: BTreeMap<SocketAddr, usize> = BTreeMap::new();
            for entry in entries {
                *packets.entry(entry.name()).or_default() += 1;
                *clients.entry(entry.addr).or_default() += 1;
            }
            let ruled: Vec<String> = filter.only.iter().chain(&filter.hidden).cloned().collect();
            let mut packet_rows: Vec<(usize, String, usize, Rule)> = packets.into_iter().map(|(name, count)| (0, name.to_string(), count, filter.rule(name))).collect();
            for name in ruled {
                if !packet_rows.iter().any(|(_, n, ..)| *n == name) {
                    let rule = filter.rule(&name);
                    packet_rows.push((0, name, 0, rule));
                }
            }
            let search = filter.packet_search.to_lowercase();
            packet_rows.retain(|(_, name, ..)| name.to_lowercase().contains(&search));
            packet_rows.sort_by(|a, b| (a.3 == Rule::Show).cmp(&(b.3 == Rule::Show)).then_with(|| a.1.cmp(&b.1)));
            for (i, row) in packet_rows.iter_mut().enumerate() {
                row.0 = i;
            }

            section(ui, "PACKET TYPES");
            ui.add(
                TextEdit::singleline(&mut filter.packet_search)
                    .hint_text(format!("{}  Search", regular::MAGNIFYING_GLASS))
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(4.);
            ScrollArea::vertical().id_salt("filter_packets").max_height(260.).auto_shrink([false, true]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.;
                if packet_rows.is_empty() {
                    let text = if search.is_empty() { "Packet types show up here once captured." } else { "No packet types match." };
                    ui.label(RichText::new(text).small().color(theme::TEXT_DIM));
                }
                if let Some((i, rule)) = rule_rows(ui, &packet_rows, false) {
                    filter.set_rule(&packet_rows[i].1, rule);
                }
            });

            if !clients.is_empty() {
                section(ui, "CLIENTS");
                let rows: Vec<(SocketAddr, String, usize, Rule)> = clients.into_iter().map(|(addr, count)| (addr, addr.to_string(), count, filter.client_rule(addr))).collect();
                ui.spacing_mut().item_spacing.y = 2.;
                if let Some((addr, rule)) = rule_rows(ui, &rows, true) {
                    filter.set_client_rule(addr, rule);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(name: &'static str, source: Source, size: usize) -> PacketInfo<'static> {
        PacketInfo {
            name,
            id: 9,
            source,
            addr: "127.0.0.1:5000".parse().unwrap(),
            size,
        }
    }

    #[test]
    fn query_combines_names_exclusions_and_qualifiers() {
        let query = Query::parse("move, text -player dir:c2s size:>10");

        assert!(query.matches(&packet("MoveActorPacket", Source::Client, 20)));
        assert!(query.matches(&packet("TextPacket", Source::Client, 20)));
        assert!(!query.matches(&packet("MovePlayerPacket", Source::Client, 20)));
        assert!(!query.matches(&packet("TextPacket", Source::Server, 20)));
        assert!(!query.matches(&packet("TextPacket", Source::Client, 5)));
        assert!(!query.matches(&packet("LoginPacket", Source::Client, 20)));
    }

    #[test]
    fn query_reports_bad_qualifiers() {
        let query = Query::parse("id:abc dir:sideways size:>big");
        assert_eq!(query.errors.len(), 3);
    }

    #[test]
    fn only_and_hidden_rules_apply_by_name() {
        let mut filter = Filter::default();
        filter.set_rule("TextPacket", Rule::Only);
        assert!(filter.matches(&packet("TextPacket", Source::Client, 1)));
        assert!(!filter.matches(&packet("LoginPacket", Source::Client, 1)));

        filter.set_rule("TextPacket", Rule::Hide);
        assert!(!filter.matches(&packet("TextPacket", Source::Client, 1)));
        assert!(filter.matches(&packet("LoginPacket", Source::Client, 1)));
    }

    #[test]
    fn only_client_rule_hides_clients_that_connect_later() {
        let mut filter = Filter::default();
        filter.set_client_rule("127.0.0.1:5000".parse().unwrap(), Rule::Only);

        let mut later = packet("TextPacket", Source::Client, 1);
        later.addr = "127.0.0.1:6000".parse().unwrap();

        assert!(filter.matches(&packet("TextPacket", Source::Client, 1)));
        assert!(!filter.matches(&later));
    }

    #[test]
    fn saved_filter_restores_packet_rules_and_query() {
        let mut filter = Filter {
            text: "text -move".into(),
            show_server: false,
            ..Default::default()
        };
        filter.set_rule("LoginPacket", Rule::Hide);
        filter.set_client_rule("127.0.0.1:5000".parse().unwrap(), Rule::Only);

        let mut restored: Filter = serde_json::from_str(&serde_json::to_string(&filter).unwrap()).unwrap();

        assert_eq!(restored.rule("LoginPacket"), Rule::Hide);
        assert!(!restored.show_server);
        assert!(restored.only_clients.is_empty());
        assert!(!restored.matches(&packet("MovePacket", Source::Client, 1)));
        assert!(restored.matches(&packet("TextPacket", Source::Client, 1)));
    }
}
