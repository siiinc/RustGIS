//! Web Data › Vantor Transform: a form for the imagery Vantor's Transform API and Raster Analytics
//! make from a collect on request (`rustgis_tiles::vantor`): the service, its script and function,
//! and the function's parameters. Add builds the GeoTIFF's address and adds it (`ui.addUrl`), read
//! in place as a cloud-optimised GeoTIFF.
//!
//! The service's credential is the login of a saved web-service connection at the service's
//! address (Data Sources › Connections): a Bearer token in the `Authorization` header for the
//! Transform API, an API key in the `maxar_api_key` query parameter for Raster Analytics. The pane
//! makes that connection when there is none and logs in to it (`connection.login`), so the
//! credential is sent only to the service's origin and is in no address that is saved or shown.
//! What is typed is handed to the command and emptied.

use std::collections::BTreeMap;

use rustgis_engine::connections::{Connection, login_origin};
use rustgis_engine::render::rustgis_tiles::{earthdata, logins, vantor};
use rustgis_engine::service_logins;
use serde_json::{Value, json};

use crate::RustGisApp;

/// What is picked and typed, and what the last login or Add said.
pub struct State {
    /// The service's id ([`vantor::SERVICES`]).
    pub service: &'static str,
    /// The service's address, where it isn't the service's own (another deployment).
    pub address: String,
    pub script: String,
    pub function: String,
    /// What is typed for each parameter, by name: a collect stays when the function changes.
    pub values: BTreeMap<String, String>,
    /// The layer's name (the collect and the function when empty).
    pub name: String,
    /// The token or key being typed. Emptied once it is handed to `connection.login`.
    pub secret: String,
    /// Remember the login on this computer (off unless ticked).
    pub remember: bool,
    /// Confirms sending the login to a plain `http://` address.
    pub insecure: bool,
    /// The connection the token last given went to, and when the token expires.
    expiry: Option<(String, i64)>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            service: vantor::TRANSFORM.id,
            address: String::new(),
            script: String::new(),
            function: String::new(),
            // The map's own coordinate system, so the image needs no reprojection to be drawn.
            values: BTreeMap::from([("crs".to_string(), "EPSG:3857".to_string())]),
            name: String::new(),
            secret: String::new(),
            remember: false,
            insecure: false,
            expiry: None,
            error: None,
            warning: None,
        }
    }
}

fn service(st: &State) -> &'static vantor::Service {
    vantor::service(st.service).unwrap_or(&vantor::TRANSFORM)
}

/// The address typed, or the service's own.
fn address(st: &State) -> &str {
    let typed = st.address.trim();
    if typed.is_empty() { service(st).address } else { typed }
}

/// The script and function picked: the service's first where what was picked isn't one of its
/// own (another service was chosen).
fn picked(st: &State) -> Option<(&'static vantor::Script, &'static vantor::Function)> {
    let service = service(st);
    let script = service.script(&st.script).or_else(|| service.scripts.first())?;
    let function = script.function(&st.function).or_else(|| script.functions.first())?;
    Some((script, function))
}

/// The saved connection whose login the service's requests carry: one at the address's origin
/// that logs in the way the service takes its credential, a logged-in one before any other.
fn connection(app: &RustGisApp) -> Option<Connection> {
    let st = &app.ui.vantor;
    let service = service(st);
    let origin = logins::origin(vantor::service_address(address(st)).ok()?)?;
    app.session
        .connections
        .connections
        .iter()
        .filter(|c| {
            service_logins::is_service(c)
                && c.auth == service.auth
                && c.key_name.eq_ignore_ascii_case(service.key_name)
                && login_origin(c).as_deref() == Some(origin.as_str())
        })
        .max_by_key(|c| service_logins::logged_in(&app.session, c))
        .cloned()
}

/// Whether `c`'s login is there to send: given this session, or remembered on this computer and
/// not logged out of.
fn usable(app: &RustGisApp, c: &Connection) -> bool {
    service_logins::logged_in(&app.session, c) || (c.remembered && !service_logins::logged_out(&app.session, c))
}

/// The Vantor Transform pane.
pub fn pane(app: &mut RustGisApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!("Vantor's Transform API and Raster Analytics make a satellite collect into imagery on request: orthorectified, pan-sharpened, NDVI. The image is a cloud-optimised GeoTIFF, read in place: the map reads only the part in view.")).weak());
    service_rows(app, ui);
    ui.separator();
    login(app, ui);
    ui.separator();
    image(app, ui);
    let st = &app.ui.vantor;
    if let Some(e) = &st.error {
        ui.label(egui::RichText::new(e).color(t.red));
    }
    if let Some(w) = &st.warning {
        ui.label(egui::RichText::new(w).small());
    }
}

/// The width of the labels' column, the same in each of the pane's grids so their boxes line up.
const LABELS: f32 = 112.0;

/// The width of a value's box beside its label.
fn field_width(ui: &egui::Ui) -> f32 {
    (ui.available_width() - LABELS - 30.0).max(120.0)
}

/// The service and its address.
fn service_rows(app: &mut RustGisApp, ui: &mut egui::Ui) {
    let field = field_width(ui);
    let st = &mut app.ui.vantor;
    let hint = service(st).address;
    egui::Grid::new("vantor_service").num_columns(2).min_col_width(LABELS).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
        ui.label(tl!("Service"));
        ui.horizontal(|ui| {
            for s in vantor::SERVICES {
                if ui.selectable_label(st.service == s.id, s.label).clicked() && st.service != s.id {
                    // Another service: its own address and login, and nothing typed for the last.
                    st.service = s.id;
                    st.address.clear();
                    st.secret.clear();
                    (st.error, st.warning) = (None, None);
                }
            }
        });
        ui.end_row();
        let label = ui.label(tl!("Address"));
        ui.add(egui::TextEdit::singleline(&mut st.address).hint_text(hint).desired_width(field))
            .on_hover_text(tl!("The service's own address unless another is given, such as a test deployment's."))
            .labelled_by(label.id);
        ui.end_row();
    });
}

/// Whether the service is logged in to, the token or key, Remember, Log In and Log Out.
fn login(app: &mut RustGisApp, ui: &mut egui::Ui) {
    let field = field_width(ui);
    let header = service(&app.ui.vantor).auth == "header";
    let connection = connection(app);
    let logged_in = connection.as_ref().is_some_and(|c| service_logins::logged_in(&app.session, c));
    match &connection {
        Some(c) if logged_in => {
            ui.label(egui::RichText::new(crate::i18n::fmt("Logged in through the connection {name}.", &[("name", &c.name)])).strong());
            if let Some((id, expires)) = &app.ui.vantor.expiry
                && *id == c.id
            {
                let date = earthdata::date_of(*expires);
                ui.label(egui::RichText::new(crate::i18n::fmt("The token expires on {date}.", &[("date", &date)])).small());
            }
            if c.remembered {
                ui.label(egui::RichText::new(tl!("It is remembered on this computer.")).small());
            }
        }
        Some(c) if usable(app, c) => {
            let text = crate::i18n::fmt(
                "The connection {name} has a login remembered on this computer: it logs in when an image is added.",
                &[("name", &c.name)],
            );
            ui.label(egui::RichText::new(text).strong());
        }
        _ => {
            ui.label(egui::RichText::new(tl!("Not logged in.")).strong());
        }
    }
    let mut enter = false;
    egui::Grid::new("vantor_login").num_columns(2).min_col_width(LABELS).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
        let st = &mut app.ui.vantor;
        let label = ui.label(if header { tl!("Bearer token") } else { tl!("API key") });
        let r = ui
            .add(egui::TextEdit::singleline(&mut st.secret).password(true).hint_text(tl!("Paste it here")).desired_width(field))
            .labelled_by(label.id);
        enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        ui.end_row();
    });
    if app.services.secrets.is_some() {
        ui.checkbox(&mut app.ui.vantor.remember, tl!("Remember on this computer")).on_hover_text(tl!(
            "Keeps the login in the system's credential store, for this connection. Unticked, it lasts until RustGIS closes."
        ));
    }
    if address(&app.ui.vantor).to_ascii_lowercase().starts_with("http://") {
        ui.checkbox(&mut app.ui.vantor.insecure, tl!("Send the login unencrypted to this http:// address"));
    }
    let ready = !app.ui.vantor.secret.trim().is_empty();
    let mut log_out = None;
    let clicked = ui
        .horizontal(|ui| {
            let clicked = ui.add_enabled(ready, egui::Button::new(tl!("Log In"))).clicked();
            if let Some(c) = connection.as_ref().filter(|_| logged_in)
                && ui
                    .button(tl!("Log Out"))
                    .on_hover_text(tl!("Ends this session's login. One remembered on this computer is kept."))
                    .clicked()
            {
                log_out = Some(c.id.clone());
            }
            clicked
        })
        .inner;
    let note = if header {
        tl!(
            "The token is the login of a saved connection at this address (Data Sources › Connections). It is sent only there, in the Authorization header, and is never kept in the project."
        )
    } else {
        tl!(
            "The key is the login of a saved connection at this address (Data Sources › Connections). It is sent only there, as the maxar_api_key parameter, and is never kept in the project or shown in an address."
        )
    };
    ui.label(egui::RichText::new(note).small().weak());
    if (clicked || enter) && ready {
        log_in(app);
    }
    if let Some(id) = log_out {
        let r = app.run("connection.logout", json!({"id": id}));
        app.ui.vantor.expiry = None;
        answered(app, r);
    }
}

/// Logs in to the service's connection with what was typed (`connection.login`), making the
/// connection first when there is none. What was typed is emptied whatever the answer.
fn log_in(app: &mut RustGisApp) {
    let r = try_log_in(app);
    answered(app, r);
}

fn try_log_in(app: &mut RustGisApp) -> Result<Value, String> {
    let st = &mut app.ui.vantor;
    let secret = std::mem::take(&mut st.secret);
    let service = service(st);
    let address = vantor::service_address(address(st))?.to_string();
    let (value, expiry) = if service.auth == "header" {
        (vantor::bearer(&secret)?, vantor::token_expiry(&secret))
    } else {
        (secret.trim().to_string(), None)
    };
    if value.is_empty() {
        return Err(tl!("Give the key").to_string());
    }
    let (remember, insecure) = (st.remember && app.services.secrets.is_some(), st.insecure);
    let id = match connection(app) {
        Some(c) => c.id,
        None => keep(app, service, &address)?,
    };
    let mut p = json!({"id": id, "apiKey": value, "remember": remember});
    if insecure && address.to_ascii_lowercase().starts_with("http://") {
        p["allowInsecure"] = json!(true);
    }
    let answer = app.run("connection.login", p)?;
    app.ui.vantor.expiry = expiry.map(|expires| (id, expires));
    Ok(answer)
}

/// Saves the service as a connection that logs in the way it takes its credential, named after
/// the service, or after its address when that name is another connection's.
fn keep(app: &mut RustGisApp, service: &vantor::Service, address: &str) -> Result<String, String> {
    let mut p = json!({"kind": "service", "address": address, "auth": service.auth, "keyName": service.key_name});
    let name = format!("Vantor {}", service.label);
    if app.session.connections.find(&name).is_none() {
        p["name"] = json!(name);
    }
    crate::connectionui::keep(app, p)
}

/// The script, the function, its parameters, the layer's name and Add.
fn image(app: &mut RustGisApp, ui: &mut egui::Ui) {
    let Some((script, function)) = picked(&app.ui.vantor) else { return };
    let service = service(&app.ui.vantor);
    let field = field_width(ui);
    let combo = |id: &'static str, text: &str| egui::ComboBox::from_id_salt(id).selected_text(text.to_string()).width(field);
    egui::Grid::new("vantor_image").num_columns(2).min_col_width(LABELS).spacing(egui::vec2(10.0, 6.0)).show(ui, |ui| {
        let st = &mut app.ui.vantor;
        ui.label(tl!("Script"));
        combo("vantor_script", script.name).show_ui(ui, |ui| {
            for s in service.scripts {
                if ui.selectable_label(s.name == script.name, s.name).clicked() {
                    st.script = s.name.to_string();
                }
            }
        });
        ui.end_row();
        ui.label(tl!("Function"));
        combo("vantor_function", function.name).show_ui(ui, |ui| {
            for f in script.functions {
                if ui.selectable_label(f.name == function.name, f.name).on_hover_text(crate::i18n::t(f.summary)).clicked() {
                    st.function = f.name.to_string();
                }
            }
        });
        ui.end_row();
        for p in function.params {
            let name = if p.required { format!("{} *", p.name) } else { p.name.to_string() };
            let label = ui.label(name).on_hover_text(crate::i18n::t(p.help));
            let value = st.values.entry(p.name.to_string()).or_default();
            let choices: &[&str] = if p.kind == vantor::Kind::Bool { &["true", "false"] } else { p.choices };
            if choices.is_empty() {
                ui.add(egui::TextEdit::singleline(value).desired_width(field)).on_hover_text(crate::i18n::t(p.help)).labelled_by(label.id);
            } else {
                // Left to the service unless one is picked.
                let shown = if value.is_empty() { tl!("Default").to_string() } else { value.clone() };
                egui::ComboBox::from_id_salt(("vantor_param", p.name)).selected_text(shown).width(field).show_ui(ui, |ui| {
                    ui.selectable_value(value, String::new(), tl!("Default"));
                    for c in choices {
                        ui.selectable_value(value, (*c).to_string(), *c);
                    }
                });
            }
            ui.end_row();
        }
        let label = ui.label(tl!("Name"));
        ui.add(egui::TextEdit::singleline(&mut st.name).hint_text(tl!("from the collect and the function")).desired_width(field))
            .labelled_by(label.id);
        ui.end_row();
    });
    if ui.button(tl!("Add")).on_hover_text(tl!("Adds the image as a layer read in place")).clicked() {
        let r = add(app).map(|()| Value::Null);
        answered(app, r);
    }
    ui.label(egui::RichText::new(crate::i18n::t(function.summary)).small().weak());
    // The address the image is asked for at (it holds no credential), once it can be built.
    if let Ok(url) = request(&app.ui.vantor) {
        ui.add(egui::Label::new(egui::RichText::new(url).small().weak().monospace()).wrap());
    }
}

/// The address of the image picked, from what is typed for its parameters.
fn request(st: &State) -> Result<String, String> {
    let (script, function) = picked(st).ok_or("this service has no scripts")?;
    let values: Vec<(&str, &str)> = st.values.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    vantor::address(address(st), script, function, &values)
}

/// Adds the image picked (`ui.addUrl`), read through the service's connection.
fn add(app: &mut RustGisApp) -> Result<(), String> {
    let st = &app.ui.vantor;
    let url = request(st)?;
    let name = match st.name.trim() {
        "" => {
            let collect = st.values.get("collect_identifier").map_or("", |c| c.trim());
            format!("{collect} {}", picked(st).map_or("", |(_, f)| f.name))
        }
        name => name.to_string(),
    };
    let connection = connection(app).filter(|c| usable(app, c)).ok_or_else(|| tl!("Log in to the service first.").to_string())?;
    // Framed once it is added: a collect is a strip a few kilometres wide.
    app.run("ui.addUrl", json!({"url": url, "name": name, "connection": connection.id, "zoom": true})).map(|_| ())
}

/// Shows what a login or Add said: its error, or the warning it gave.
fn answered(app: &mut RustGisApp, r: Result<Value, String>) {
    let st = &mut app.ui.vantor;
    (st.error, st.warning) = match r {
        Ok(v) => (None, v.get("warning").and_then(Value::as_str).map(str::to_string)),
        Err(e) => (Some(e), None),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use std::io::{Read, Write};
    use std::sync::{Arc, Mutex};

    type Harness = egui_kittest::Harness<'static, RustGisApp>;

    fn harness() -> Harness {
        egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 860.0)).with_max_steps(64).with_step_dt(0.05).build_ui_state(
            |ui, app: &mut RustGisApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            RustGisApp::with_sample(),
        )
    }

    /// A stand-in for a service on 127.0.0.1: a cloud-optimised GeoTIFF, by range, for a request
    /// whose head (lower-cased) holds `credential`, and 401 for any other. Logs each request's
    /// first line and whether it carried the credential.
    fn serve(credential: &'static str) -> (String, Arc<Mutex<Vec<(String, bool)>>>) {
        let body = rustgis_engine::data::raster::cog::example(512, 256);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let kept = log.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut buf = [0u8; 8192];
                let read = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..read]).to_ascii_lowercase();
                let allowed = head.contains(credential);
                kept.lock().unwrap().push((head.lines().next().unwrap_or("").to_string(), allowed));
                let range = head.lines().find_map(|l| l.strip_prefix("range: bytes=")).and_then(|r| {
                    let (a, b) = r.trim().split_once('-')?;
                    Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?))
                });
                let (status, part) = match range {
                    _ if !allowed => ("401 Unauthorized", &body[..0]),
                    _ if head.starts_with("head ") => ("200 OK", &body[..0]),
                    Some((a, b)) if a < body.len() => ("206 Partial Content", &body[a..=b.min(body.len() - 1)]),
                    _ => ("200 OK", &body[..]),
                };
                let length = if head.starts_with("head ") && allowed { body.len() } else { part.len() };
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: image/tiff\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
                let _ = s.write_all(part);
            }
        });
        (base, log)
    }

    /// Runs frames until a raster layer named `name`, read in place, is on the map, and answers
    /// its address.
    fn added(h: &mut Harness, name: &str) -> String {
        for _ in 0..300 {
            h.step();
            if let Some(raster) = h.state().session.map().layers.iter().find(|l| l.name == name).and_then(|l| l.raster.as_ref()) {
                assert!(raster.in_place, "{name} is read in place");
                return raster.path.clone().unwrap_or_default();
            }
            std::thread::sleep(std::time::Duration::from_millis(20)); // test-wait: the probe and header are read off the frame, with no signal a test can wait on
        }
        let notices: Vec<_> = h.state().session.notices.iter().map(|n| n.text.clone()).collect();
        panic!("{name} was never added: {notices:?}");
    }

    /// The Transform API: the pane refuses to add before a login, and a token that isn't one
    /// word; a login makes the connection (an `Authorization` header login) and logs in to it;
    /// Add reads the image in place, every request carrying the token, which is in no address.
    #[test]
    fn a_bearer_token_logs_in_and_the_image_is_read_in_place() {
        let (base, log) = serve("authorization: bearer test.token-1\r\n");
        let mut h = harness();
        h.step();
        h.state_mut().run("ui.dialog", json!({"name": "vantorTransform"})).unwrap();
        h.step();
        assert_eq!(h.state().ui.web_data, "vantorTransform");
        h.get_by_label("Not logged in.");
        let st = &mut h.state_mut().ui.vantor;
        st.address = format!("{base}/v1/ipe/");
        st.values.insert("collect_identifier".into(), "X1".into());
        assert!(add(h.state_mut()).unwrap_err().contains("Log in"), "nothing is added without a login");

        h.state_mut().ui.vantor.secret = "two words".into();
        log_in(h.state_mut());
        let st = &h.state().ui.vantor;
        assert!(st.error.as_deref().is_some_and(|e| e.contains("one word")), "{:?}", st.error);
        assert!(st.secret.is_empty(), "what was typed is emptied whatever the answer");
        assert!(connection(h.state()).is_none(), "a refused token saves no connection");

        // A plain http:// address takes the login only once that is confirmed.
        h.state_mut().ui.vantor.secret = "Bearer test.token-1".into();
        log_in(h.state_mut());
        assert!(h.state().ui.vantor.error.as_deref().is_some_and(|e| e.contains("http://")), "{:?}", h.state().ui.vantor.error);
        let st = &mut h.state_mut().ui.vantor;
        (st.secret, st.insecure) = ("Bearer test.token-1".into(), true);
        log_in(h.state_mut());
        let st = &h.state().ui.vantor;
        assert!(st.error.is_none() && st.secret.is_empty(), "{:?}", st.error);
        let c = connection(h.state()).unwrap();
        assert_eq!(
            (c.kind.as_str(), c.auth.as_str(), c.key_name.as_str(), c.address.as_str(), c.name.as_str()),
            ("service", "header", "Authorization", format!("{base}/v1/ipe").as_str(), "Vantor Transform API")
        );
        assert!(service_logins::logged_in(&h.state().session, &c));
        h.step();
        h.get_by_label("Logged in through the connection Vantor Transform API.");

        add(h.state_mut()).unwrap();
        let url = added(&mut h, "X1 ortho");
        assert_eq!(
            rustgis_engine::data::login_address::inner(&url),
            format!("{base}/v1/ipe/ortho-with-overviews/geotiff?function=ortho&p=collect_identifier=%22X1%22&p=crs=%22EPSG%3A3857%22")
        );
        let asked = log.lock().unwrap().clone();
        assert!(
            asked.iter().any(|(line, _)| line.starts_with("head ")) && asked.iter().any(|(line, _)| line.starts_with("get ")),
            "{asked:?}"
        );
        assert!(asked.iter().all(|(_, carried)| *carried), "every request carries the token: {asked:?}");
        let project = serde_json::to_string(&h.state().session.project).unwrap();
        assert!(!project.contains("test.token-1") && !url.contains("test.token-1"), "the token is in no address");

        // Logged out, the pane asks for a login again.
        let id = c.id.clone();
        h.state_mut().run("connection.logout", json!({"id": id})).unwrap();
        assert!(add(h.state_mut()).unwrap_err().contains("Log in"));
    }

    /// Raster Analytics: its key is a query login, sent as `maxar_api_key` with every request
    /// and left out of the layer's address.
    #[test]
    fn an_api_key_is_sent_as_a_query_login() {
        let (base, log) = serve("maxar_api_key=k-123");
        let mut h = harness();
        h.step();
        let st = &mut h.state_mut().ui.vantor;
        st.service = vantor::RASTER_ANALYTICS.id;
        st.address = format!("{base}/analytics/v1/raster");
        (st.secret, st.insecure) = ("k-123".into(), true);
        st.values.insert("collect_identifier".into(), "X2".into());
        st.name = "Analytics".into();
        log_in(h.state_mut());
        assert!(h.state().ui.vantor.error.is_none(), "{:?}", h.state().ui.vantor.error);
        let c = connection(h.state()).unwrap();
        assert_eq!((c.auth.as_str(), c.key_name.as_str(), c.name.as_str()), ("query", "maxar_api_key", "Vantor Raster Analytics"));

        add(h.state_mut()).unwrap();
        let url = added(&mut h, "Analytics");
        assert!(!url.contains("k-123") && !url.to_ascii_lowercase().contains("maxar_api_key"), "{url}");
        let asked = log.lock().unwrap().clone();
        assert!(!asked.is_empty() && asked.iter().all(|(_, carried)| *carried), "every request carries the key: {asked:?}");
    }

    /// A function's parameters follow the script and function picked, and what another service
    /// doesn't offer falls back to its first.
    #[test]
    fn the_form_follows_the_service_script_and_function() {
        let mut st = State::default();
        assert_eq!(st.values.get("crs").map(String::as_str), Some("EPSG:3857"));
        assert_eq!(picked(&st).map(|(s, f)| (s.name, f.name)), Some(("ortho-with-overviews", "ortho")));
        (st.script, st.function) = ("ortho".into(), "pansharp_ortho".into());
        assert_eq!(picked(&st).map(|(s, f)| (s.name, f.name)), Some(("ortho", "pansharp_ortho")));
        assert_eq!(address(&st), vantor::TRANSFORM.address);
        // Raster Analytics has NDVI; the Transform API doesn't, and shows its first script.
        (st.service, st.script, st.function) = (vantor::RASTER_ANALYTICS.id, "ndvi-image".into(), "ndvi_dra".into());
        assert_eq!(picked(&st).map(|(s, f)| (s.name, f.name)), Some(("ndvi-image", "ndvi_dra")));
        assert_eq!(address(&st), vantor::RASTER_ANALYTICS.address);
        st.service = vantor::TRANSFORM.id;
        assert_eq!(picked(&st).map(|(s, f)| (s.name, f.name)), Some(("ortho-with-overviews", "ortho")));
        // The address is built only once what is required is typed.
        assert!(request(&st).unwrap_err().contains("collect_identifier is required"));
        st.values.insert("collect_identifier".into(), "X".into());
        st.values.insert("dra".into(), "false".into());
        assert!(request(&st).unwrap().ends_with("&p=crs=%22EPSG%3A3857%22&p=dra=false"));
    }
}
