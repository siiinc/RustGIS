//! The Symbology pane's **Copy From** and **Copy To** menus, two small buttons on the row of the
//! layer's name (in view however long its classes make the pane, and taking no height from its
//! controls). Copy From draws the layer in the pane as another layer of the map is drawn, and
//! Copy To another as it is (`layer.copySymbology`, one Undo step). Each lists every other layer
//! of the map: those that have what the symbology reads first, then the rest, each switched off
//! with the reason the command would refuse it for
//! (`rustgis_engine::cmd::symbology_copy::misfit`), so nothing is left out unexplained. The pane
//! keeps nothing of its own: the lists are read from the map as a menu is drawn.
//!
//! They are two menus rather than one with two submenus: in egui 0.37 a submenu whose entries are
//! all switched off (a raster's list on a map of feature layers) closes its menu when it is
//! opened by a click.

use egui::{RichText, Ui};
use rustgis_engine::cmd::symbology_copy::misfit;
use rustgis_engine::map::Layer;
use serde_json::json;

use crate::RustGisApp;
use crate::theme::{Tokens, semibold};

/// The widest a menu is drawn, so a reason wraps under its layer instead of stretching the menu.
const MENU_W: f32 = 300.0;
/// The tallest a menu's list is drawn before it scrolls (a map can hold hundreds of layers).
const MENU_H: f32 = 360.0;

/// Which way a menu copies: into the pane's layer, or from it.
#[derive(Clone, Copy)]
enum Way {
    From,
    To,
}

/// The Symbology pane's heading for the layer `l` it shows: its name, and after it on the same
/// row the two menus as small buttons (on the next row under a name too long to share one). The
/// row is no taller than the name alone, so the controls below stay where they were.
pub(crate) fn heading(app: &mut RustGisApp, ui: &mut Ui, l: &Layer, t: &Tokens) {
    use egui::containers::menu::MenuButton;
    ui.scope(|ui| {
        // A row of its text's height, not of a full-size button's.
        ui.spacing_mut().interact_size.y = 0.0;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&l.name).font(semibold(14.0)));
            for (label, hint, way) in [
                ("Copy From ▾", "Draw this layer as another layer of the map is drawn", Way::From),
                ("Copy To ▾", "Draw another layer of the map as this layer is drawn", Way::To),
            ] {
                let (button, _) = MenuButton::from_button(egui::Button::new(tl!(label)).small()).ui(ui, |ui| menu(app, ui, l, way, t));
                button.on_hover_text(tl!(hint));
            }
        });
    });
}

/// One of the two lists: the map's other layers, top first as the Layers pane lists them; those
/// that fit as buttons, then the others switched off, each with why.
fn menu(app: &mut RustGisApp, ui: &mut Ui, l: &Layer, way: Way, t: &Tokens) {
    ui.set_max_width(MENU_W);
    // (id, name, why not) of each other layer, read once for this frame.
    let others: Vec<(u32, String, Option<String>)> = app
        .session
        .map()
        .layers
        .iter()
        .filter(|other| other.id != l.id)
        .map(|other| {
            let why = match way {
                Way::From => misfit(other, l),
                Way::To => misfit(l, other),
            };
            (other.id, other.name.clone(), why)
        })
        .collect();
    if others.is_empty() {
        ui.label(RichText::new(tl!("The map has no other layer.")).color(t.text_dim));
        return;
    }
    egui::ScrollArea::vertical().max_height(MENU_H).show(ui, |ui| {
        let mut picked = None;
        for (id, name, _) in others.iter().filter(|(_, _, why)| why.is_none()) {
            if ui.button(name).clicked() {
                picked = Some(*id);
            }
        }
        let fit = others.iter().filter(|(_, _, why)| why.is_none()).count();
        if fit < others.len() {
            if fit > 0 {
                ui.separator();
            }
            let heading = match way {
                Way::From => tl!("Not consistent with this layer"),
                Way::To => tl!("Can't take this symbology"),
            };
            ui.label(RichText::new(heading).size(11.5).color(t.text_dim));
            for (_, name, why) in &others {
                let Some(why) = why else { continue };
                ui.add_enabled(false, egui::Button::new(name)).on_disabled_hover_text(why);
                reason(ui, why, t);
            }
        }
        if let Some(other) = picked {
            let params = match way {
                Way::From => json!({"layer": l.id, "from": other}),
                Way::To => json!({"layer": other, "from": l.id}),
            };
            // A layer that changed since the list was drawn is refused by the command, which says why.
            if let Err(error) = app.run("layer.copySymbology", params) {
                app.session.notify(error, true);
            }
            ui.close();
        }
    });
}

/// Why a layer is switched off, under its name and in line with it. A menu stretches its rows
/// to its width, which would space a wrapped sentence's words apart: the reason is laid out
/// from the left instead.
fn reason(ui: &mut Ui, why: &str, t: &Tokens) {
    let indent = ui.spacing().button_padding.x;
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::NONE.inner_margin(egui::Margin { left: indent as i8, right: 0, top: 0, bottom: 3 }).show(ui, |ui| {
            ui.add(egui::Label::new(RichText::new(why).size(11.0).color(t.text_dim)).wrap());
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::accesskit::Role;
    use egui_kittest::kittest::{NodeT, Queryable};
    use rustgis_engine::map::Renderer;

    type Harness = egui_kittest::Harness<'static, RustGisApp>;

    /// The pane's heading alone, for the layer named `active` of the sample map as `set` leaves
    /// it (without the Layers pane, whose rows bear the layers' names too).
    fn harness(active: &str, set: impl FnOnce(&mut RustGisApp)) -> Harness {
        let mut app = RustGisApp::with_sample();
        set(&mut app);
        app.session.active_layer = app.session.map().layer_by_name(active).map(|l| l.id);
        let mut h = egui_kittest::Harness::builder()
            .with_size(egui::vec2(700.0, 700.0))
            .with_max_steps(64)
            // The app's fonts: the menu buttons' ▾ isn't in egui's own.
            .with_context_setup(|ctx| crate::theme::install_fonts(ctx, &[], None))
            .build_ui_state(
                |ui, app: &mut RustGisApp| {
                    let t = Tokens::get(ui.ctx());
                    let Some(l) = app.session.active_layer.and_then(|id| app.session.map().layer(id)).cloned() else { return };
                    heading(app, ui, &l, &t);
                },
                app,
            );
        h.run_steps(3);
        h
    }

    /// Opens the menu named `menu` (Copy From or Copy To).
    fn open(h: &mut Harness, menu: &str) {
        h.get_by_label(menu).click();
        // A menu is laid out, then placed: its rows are where a click finds them a few frames on.
        h.run_steps(6);
    }

    /// The button of the layer named `layer` in the open list, and whether it is switched off.
    fn listed<'a>(h: &'a Harness, layer: &'a str) -> (egui_kittest::Node<'a>, bool) {
        let button = h.get_by_role_and_label(Role::Button, layer);
        let off = button.accesskit_node().is_disabled();
        (button, off)
    }

    fn renderer(app: &RustGisApp, layer: &str) -> Renderer {
        app.session.map().layer_by_name(layer).unwrap().renderer.clone()
    }

    /// Copy From lists the layers whose symbology this one can take as buttons and the others
    /// switched off with the reason; picking one copies its symbology in one Undo step.
    #[test]
    fn copy_from_lists_what_fits_and_says_why_the_rest_do_not() {
        let mut h = harness("Lakes", |app| {
            app.run("layer.symbology", json!({"layer": "Countries", "type": "graduated", "field": "pop_est"})).unwrap();
        });
        let own = renderer(h.state(), "Lakes");
        open(&mut h, "Copy From ▾");
        // Points and lines can't draw a polygon layer; Countries classes by a field Lakes hasn't.
        for (layer, why) in [
            ("Cities", "Cities is a point layer and Lakes is a polygon layer"),
            ("Rivers", "Rivers is a polyline layer and Lakes is a polygon layer"),
            ("Countries", "Lakes has no field 'pop_est' (the Graduated Colors field of Countries)"),
        ] {
            assert!(listed(&h, layer).1, "{layer} is listed, switched off");
            assert!(h.query_by_label(why).is_some(), "{why}");
        }
        // With a single symbol, Countries reads no field: it fits.
        h.state_mut().run("layer.symbology", json!({"layer": "Countries", "type": "single"})).unwrap();
        h.run_steps(3);
        let countries = renderer(h.state(), "Countries");
        assert_ne!(own, countries);
        let steps = h.state().session.undo_depth();
        let (countries_button, off) = listed(&h, "Countries");
        assert!(!off, "Countries fits now");
        countries_button.click();
        h.run_steps(4);
        assert_eq!(renderer(h.state(), "Lakes"), countries);
        assert_eq!(h.state().session.undo_depth(), steps + 1, "one Undo step");
        assert!(h.query_by_label("Not consistent with this layer").is_none(), "the menu closed");
        h.state_mut().run("edit.undo", serde_json::Value::Null).unwrap();
        assert_eq!(renderer(h.state(), "Lakes"), own);
    }

    /// Copy To gives this layer's symbology to another that can take it, and switches off those
    /// that can't with the reason.
    #[test]
    fn copy_to_styles_another_layer() {
        let mut h = harness("Countries", |app| {
            app.run("layer.symbology", json!({"layer": "Countries", "type": "single"})).unwrap();
            app.run("layer.color", json!({"layer": "Countries", "color": "#336699"})).unwrap();
        });
        let (own, countries) = (renderer(h.state(), "Lakes"), renderer(h.state(), "Countries"));
        assert_ne!(own, countries);
        open(&mut h, "Copy To ▾");
        assert!(h.query_by_label("Can't take this symbology").is_some());
        assert!(h.query_by_label("Countries is a polygon layer and Cities is a point layer").is_some());
        let (lakes, off) = listed(&h, "Lakes");
        assert!(!off, "Lakes can take a single symbol");
        lakes.click();
        h.run_steps(4);
        assert_eq!(renderer(h.state(), "Lakes"), countries);
        // The pane still shows the layer it showed.
        let s = &h.state().session;
        assert_eq!(s.active_layer, s.map().layer_by_name("Countries").map(|l| l.id));
    }

    /// The Symbology pane shows the menus for a feature layer and for a raster, whose list on a
    /// map of feature layers has every layer switched off, each with its reason.
    #[test]
    fn the_symbology_pane_shows_the_menus_for_feature_layers_and_rasters() {
        use rustgis_engine::data::raster::{ColorHint, Crs, GeoTransform, Raster, geotiff};
        let mut app = RustGisApp::with_sample();
        app.ui.open_right(crate::RightPane::Symbology);
        let mut h = egui_kittest::Harness::builder().with_size(egui::vec2(1400.0, 800.0)).with_max_steps(64).build_ui_state(
            |ui, app: &mut RustGisApp| {
                let ctx = ui.ctx().clone();
                app.logic(&ctx);
                app.ui(ui);
            },
            app,
        );
        h.run_steps(6);
        assert!(h.state().session.active_layer.and_then(|id| h.state().session.map().layer(id)).is_some_and(|l| !l.is_raster()));
        assert!(h.query_by_label("Copy From ▾").is_some() && h.query_by_label("Copy To ▾").is_some());
        let gt = GeoTransform { origin_x: 0.0, origin_y: 4.0, pixel_width: 1.0, pixel_height: -1.0 };
        let r = Raster::new("Relief", 4, 4, vec![(0..16).map(|i| i as f32).collect()], gt, Crs::LonLat, None, ColorHint::Gray, "Float32")
            .unwrap();
        let bytes = geotiff::write(&r, 0).unwrap();
        let layer = rustgis_engine::map::RasterLayer::from_bytes(r, "relief.tif".to_string(), bytes);
        rustgis_engine::cmd::raster::add_raster(&mut h.state_mut().session, "Relief", layer).unwrap();
        h.run_steps(6);
        assert!(h.state().session.active_layer.and_then(|id| h.state().session.map().layer(id)).is_some_and(|l| l.is_raster()));
        assert!(h.query_by_label("Copy From ▾").is_some() && h.query_by_label("Copy To ▾").is_some());
        // A raster and a feature layer aren't drawn alike: every sample layer is listed, switched off.
        open(&mut h, "Copy From ▾");
        for layer in ["Cities", "Rivers", "Lakes", "Countries"] {
            assert!(h.query_by_label(&format!("{layer} is a feature layer and Relief is a raster")).is_some(), "{layer}");
        }
        // The menu stays open on a list with nothing to pick, to be read.
        h.run_steps(6);
        assert!(h.query_by_label("Not consistent with this layer").is_some());
    }
}
