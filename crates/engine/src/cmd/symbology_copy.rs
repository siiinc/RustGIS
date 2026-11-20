//! Style › Copy Symbology (`layer.copySymbology`): one layer drawn as another is.
//!
//! A layer takes another's symbology only when it has what that symbology reads ([`misfit`]): the
//! two are the same kind of layer (both feature layers or both rasters); feature layers have the
//! same geometry type, and every field the symbology reads is in the layer taking it, a number
//! where a number is read and otherwise of a type whose values print alike; a raster has every
//! band the symbology draws, a colour table for Colormap and whole numbers for Unique Values. A
//! copy is whole or refused with the reason, never made in part.
//!
//! The symbology is what the Symbology pane sets: a feature layer's renderer with its classes,
//! its stacked strokes and fills, No Fill, marker rotation, size by field, the By Expression
//! settings, its diagram, reference scale and transparency; a raster's renderer, stretch, DRA,
//! resampling, enhancement, masked values and transparency. Labels, the visibility range, blend,
//! masking, display filters and the definition query stay the layer's own, as does the angle a
//! raster is turned by.
//!
//! Classes, size ranges, custom stretch bounds and masked values are copied as they are, so the two
//! layers share one legend; they aren't made again from the values of the layer taking them. The
//! work is the symbology's size, whatever the layers hold: no feature or pixel is read, so it runs
//! where it is called.

use rustgis_data::query::{parse_fields, reads_field, uses_area};
use rustgis_data::raster::ColorHint;
use rustgis_data::{Field, FieldType, GeometryType};
use rustgis_map::raster::{RasterLayer, RasterRenderer, RasterStyle};
use rustgis_map::symbol_by::{SizeBy, SymbolBy};
use rustgis_map::{Layer, Renderer};
use serde_json::{Value, json};

use super::need_layer;
use crate::{CmdResult, CommandSpec as C, Session};

pub fn commands() -> Vec<C> {
    vec![
        C::new("layer.copySymbology", "Copy Symbology From Layer", "Style › Drawing", copy)
            .inputs(copy_inputs)
            .roles(&[])
            .effects(crate::policy::Effects::NONE)
            .when(need_layer)
            .params(
                r#"{"layer"?: name|id (the layer that takes the symbology; default: the active layer), "from": name|id (the layer whose symbology it takes: the same kind of layer, feature or raster; a feature layer of the same geometry type, every field the symbology reads being in the layer that takes it; a raster with every band it draws)} → the two layers' names and the symbology copied; refused with the reason when the layer hasn't what the symbology reads"#,
            ),
    ]
}

/// The `"from"` selector as a `"layer"` one, so both resolve by the same rules: a name, else an id.
fn from_selector(p: &Value) -> Result<Value, String> {
    match p.get("from") {
        Some(v @ Value::String(name)) if !name.trim().is_empty() => Ok(json!({"layer": v})),
        Some(v @ Value::Number(_)) => Ok(json!({"layer": v})),
        _ => Err("from is required: the name or id of the layer whose symbology to copy".into()),
    }
}

/// Binds the layer styled and the layer its symbology comes from, without reading either.
fn copy_inputs(input: &super::InputContext<'_>, p: &Value) -> Result<super::Inputs, String> {
    let from = from_selector(p)?;
    let mut inputs = super::Inputs::single_layer(input, p)?;
    inputs.extend(super::Inputs::single_layer(input, &from)?);
    Ok(inputs)
}

fn copy(s: &mut Session, p: &Value) -> CmdResult {
    let from = s.layer_id(&from_selector(p)?)?;
    let to = s.layer_id(p)?;
    let m = s.map();
    let (source, target) = (m.layer(from).ok_or("no such layer")?, m.layer(to).ok_or("no such layer")?);
    if from == to {
        return Err(format!("{} has this symbology already: name another layer in \"from\"", target.name));
    }
    if let Some(why) = misfit(source, target) {
        return Err(why);
    }
    let copied = Copied::of(source);
    let (from_name, to_name) = (source.name.clone(), target.name.clone());
    let l = s.map_mut().layer_mut(to).ok_or("no such layer")?;
    let reads_area = copied.give(l);
    let symbology = match &l.raster {
        Some(r) => r.style.renderer.label(),
        None => l.renderer.kind_label(),
    };
    // Areas the copied expressions can't measure here are said, as Apply Style says them.
    let unmeasured = if reads_area { super::design::display_area_message(l) } else { None };
    // Said, as the layer styled needn't be the one the Symbology pane shows.
    s.notify(format!("{to_name} is drawn with the symbology of {from_name}"), false);
    if let Some(notice) = unmeasured {
        s.notify(notice, false);
    }
    Ok(json!({"layer": to_name, "from": from_name, "symbology": symbology}))
}

/// A layer's symbology, apart from the layer.
enum Copied {
    Features(Box<FeatureSymbology>),
    Raster { style: Box<RasterStyle>, transparency: f32 },
}

/// What the Symbology pane sets on a feature layer.
struct FeatureSymbology {
    renderer: Renderer,
    line_strokes: Vec<rustgis_map::LineStroke>,
    fill_layers: Vec<rustgis_map::fills::FillLayer>,
    stroke_levels: bool,
    no_fill: Option<rustgis_map::NoFill>,
    hidden_fills: Option<rustgis_map::HiddenFills>,
    marker_rotation: Option<rustgis_map::MarkerRotation>,
    size_by: Option<SizeBy>,
    symbol_by: SymbolBy,
    diagram: Option<rustgis_map::DiagramSettings>,
    reference_scale: Option<f64>,
    transparency: f32,
}

impl Copied {
    /// The symbology `l` draws with. A setting kept without being drawn isn't part of it
    /// ([`turns_markers`], [`sizes_by_field`]).
    fn of(l: &Layer) -> Copied {
        if let Some(r) = &l.raster {
            return Copied::Raster { style: Box::new(r.style.clone()), transparency: l.transparency };
        }
        let mut symbol_by = l.symbol_by.clone();
        if !turns_markers(l) {
            symbol_by.size = None;
            symbol_by.angle = None;
        }
        Copied::Features(Box::new(FeatureSymbology {
            renderer: l.renderer.clone(),
            line_strokes: l.line_strokes.clone(),
            fill_layers: l.fill_layers.clone(),
            stroke_levels: l.stroke_levels,
            no_fill: l.no_fill,
            hidden_fills: l.hidden_fills.clone(),
            marker_rotation: l.marker_rotation.clone().filter(|_| turns_markers(l)),
            size_by: l.size_by.clone().filter(|_| sizes_by_field(l)),
            symbol_by,
            diagram: l.diagram.clone(),
            reference_scale: l.reference_scale,
            transparency: l.transparency,
        }))
    }

    /// Gives `l` the symbology, in place of its own ([`misfit`] has passed). Whether an expression
    /// of it measures areas on `l`.
    fn give(self, l: &mut Layer) -> bool {
        match self {
            Copied::Raster { style, transparency } => {
                if let Some(r) = l.raster.as_mut() {
                    // The angle a raster is turned by is where it lies, not how it is drawn.
                    let rotation = r.style.rotation;
                    r.style = *style;
                    r.style.rotation = rotation;
                }
                l.transparency = transparency;
                false
            }
            Copied::Features(f) => {
                let f = *f;
                let reads_area = f
                    .renderer
                    .expression()
                    .into_iter()
                    .chain(expressions(&f.symbol_by).map(|(_, text, _)| text))
                    .any(|text| parse_fields(text, &l.dataset).is_ok_and(|e| uses_area(&e)));
                l.renderer = f.renderer;
                l.line_strokes = f.line_strokes;
                l.fill_layers = f.fill_layers;
                l.stroke_levels = f.stroke_levels;
                // No Fill as on the layer copied: hidden alike, and Restore Fill shows the same.
                l.no_fill = f.no_fill;
                l.hidden_fills = f.hidden_fills;
                l.marker_rotation = f.marker_rotation;
                l.size_by = f.size_by;
                l.symbol_by = f.symbol_by;
                l.diagram = f.diagram;
                l.reference_scale = f.reference_scale;
                l.transparency = f.transparency;
                reads_area
            }
        }
    }
}

/// The By Expression settings that are set: what each is called, its text, and whether it is of
/// markers alone (a size, an angle), so drawn on point layers only.
fn expressions(by: &SymbolBy) -> impl Iterator<Item = (&'static str, &str, bool)> {
    [
        ("the size expression", &by.size, true),
        ("the fill expression", &by.fill, false),
        ("the outline expression", &by.stroke, false),
        ("the outline width expression", &by.stroke_width, false),
        ("the angle expression", &by.angle, true),
    ]
    .into_iter()
    .filter_map(|(what, text, markers)| text.as_deref().map(|text| (what, text, markers)))
}

/// What a symbology reads of its layer's fields.
enum Reads<'a> {
    /// A field read as a number.
    Number(&'a str),
    /// A field whose values are matched to classes as they print.
    Values(&'a str),
    /// An expression, which reads the fields it names.
    Expression(&'a str),
}

/// Whether `l` draws markers, which alone are turned by a field's angle and sized or turned by an
/// expression: on lines and polygons those settings are kept without being drawn.
fn turns_markers(l: &Layer) -> bool {
    l.geometry_type() == GeometryType::Point
}

/// Whether `l`'s size by field is drawn: kept under a renderer without classes, it isn't.
fn sizes_by_field(l: &Layer) -> bool {
    l.size_by.is_some() && SizeBy::applies_to(l)
}

/// Everything the symbology `l` draws with ([`Copied::of`]) reads of its fields, each with what
/// it is called.
fn reads(l: &Layer) -> Vec<(&'static str, Reads<'_>)> {
    let mut out = Vec::new();
    match &l.renderer {
        Renderer::Single(_) => {}
        Renderer::Unique { expression: Some(e), .. } => out.push(("the Unique Values expression", Reads::Expression(e))),
        Renderer::Unique { field, .. } => out.push(("the Unique Values field", Reads::Values(field))),
        Renderer::Graduated { field, .. } => out.push(("the Graduated Colors field", Reads::Number(field))),
        Renderer::Proportional { field, .. } => out.push(("the Proportional Symbols field", Reads::Number(field))),
    }
    let markers = turns_markers(l);
    if let Some(r) = l.marker_rotation.as_ref().filter(|_| markers) {
        out.push(("the marker rotation field", Reads::Number(&r.field)));
    }
    if let Some(b) = l.size_by.as_ref().filter(|_| sizes_by_field(l)) {
        out.push(("the size by field", Reads::Number(&b.field)));
    }
    let drawn = expressions(&l.symbol_by).filter(|(_, _, of_markers)| markers || !of_markers);
    out.extend(drawn.map(|(what, text, _)| (what, Reads::Expression(text))));
    if let Some(d) = &l.diagram {
        out.extend(d.categories.iter().map(|c| ("a diagram field", Reads::Number(c.field.as_str()))));
    }
    out
}

/// Whether classes made from a field of type `a` match the values of one of type `b`: values are
/// matched as they print, and a whole number prints alike in a Long and a Double field.
fn print_alike(a: FieldType, b: FieldType) -> bool {
    a == b || (a.is_numeric() && b.is_numeric())
}

/// Why the layer `to` can't take the symbology of `from`, or `None` when it can: the rule this
/// module's documentation gives, in the order a person would check it (the kind of layer, the
/// geometry type, then each field or band the symbology reads). Both are layers of one map; a
/// layer and itself aren't told apart here.
pub fn misfit(from: &Layer, to: &Layer) -> Option<String> {
    match (&from.raster, &to.raster) {
        (Some(a), Some(b)) => return raster_misfit(&from.name, a, &to.name, b),
        (Some(_), None) => return Some(format!("{} is a raster and {} is a feature layer", from.name, to.name)),
        (None, Some(_)) => return Some(format!("{} is a feature layer and {} is a raster", from.name, to.name)),
        (None, None) => {}
    }
    for l in [from, to] {
        if l.graphics {
            return Some(format!("{} is a graphics layer: each graphic has a style of its own", l.name));
        }
        if l.geometry_type() == GeometryType::None {
            return Some(format!("{} is a table: it has no symbols", l.name));
        }
    }
    let (a, b) = (from.geometry_type(), to.geometry_type());
    if a != b {
        let kind = |g: GeometryType| g.label().to_ascii_lowercase();
        return Some(format!("{} is a {} layer and {} is a {} layer", from.name, kind(a), to.name, kind(b)));
    }
    reads(from).into_iter().find_map(|(what, read)| field_misfit(from, to, what, &read))
}

/// The field of `l` named `name`, any case, as [`rustgis_data::Dataset::field_index`] finds it.
fn field<'a>(l: &'a Layer, name: &str) -> Option<&'a Field> {
    l.dataset.field_index(name).and_then(|i| l.dataset.fields.get(i))
}

/// Why `to` hasn't what one part of `from`'s symbology (`what`) reads, if it hasn't.
fn field_misfit(from: &Layer, to: &Layer, what: &str, read: &Reads<'_>) -> Option<String> {
    let missing = |name: &str| format!("{} has no field '{name}' ({what} of {})", to.name, from.name);
    let unlike = |here: &Field, there: &Field| {
        format!(
            "'{}' is a {} field in {} and a {} field in {}: {what} wouldn't match its values",
            here.name,
            here.ty.label(),
            to.name,
            there.ty.label(),
            from.name
        )
    };
    match read {
        Reads::Number(name) => match field(to, name) {
            None => Some(missing(name)),
            Some(f) if !f.ty.is_numeric() => {
                Some(format!("'{}' is a {} field in {}: {what} of {} is read as a number", f.name, f.ty.label(), to.name, from.name))
            }
            Some(_) => None,
        },
        Reads::Values(name) => {
            let Some(here) = field(to, name) else { return Some(missing(name)) };
            // A field its own layer has lost is matched by name alone.
            field(from, name).filter(|there| !print_alike(there.ty, here.ty)).map(|there| unlike(here, there))
        }
        Reads::Expression(text) => {
            // The fields it reads where it was written, each needed here as it is there. One that
            // doesn't read there (a field gone since) is checked as text alone, below.
            if let Ok(e) = parse_fields(text, &from.dataset) {
                for (_, there) in from.dataset.fields.iter().enumerate().filter(|(i, _)| reads_field(&e, *i)) {
                    match field(to, &there.name) {
                        None => return Some(missing(&there.name)),
                        Some(here) if !print_alike(there.ty, here.ty) => return Some(unlike(here, there)),
                        Some(_) => {}
                    }
                }
            }
            parse_fields(text, &to.dataset).err().map(|e| format!("{what} of {} doesn't read on {}: {e}", from.name, to.name))
        }
    }
}

/// Why the raster `b` (the layer `to`) can't be drawn as `a` (the layer `from`) is, if it can't:
/// its pixels aren't read, it hasn't a band `a` draws, or it lacks what `a`'s renderer needs.
fn raster_misfit(from: &str, a: &RasterLayer, to: &str, b: &RasterLayer) -> Option<String> {
    let Some(data) = b.data.as_ref() else {
        return Some(format!("{to}'s pixels aren't read, so its bands can't be checked"));
    };
    let count = data.band_count();
    let renderer = &a.style.renderer;
    if let Some(band) = renderer.bands().into_iter().find(|band| *band >= count) {
        let bands = if count == 1 { "1 band".to_string() } else { format!("{count} bands") };
        return Some(format!("{from} draws band {} and {to} has {bands}", band.saturating_add(1)));
    }
    match renderer {
        RasterRenderer::Colormap if !matches!(data.color, ColorHint::Palette(_)) => {
            Some(format!("{from} draws its own colour table and {to} has none"))
        }
        RasterRenderer::UniqueValues { band, .. } if data.bands.get(*band).is_none_or(|b| b.stats.categories.is_none()) => Some(format!(
            "{from} draws Unique Values and band {} of {to} isn't whole numbers with at most 256 distinct values",
            band.saturating_add(1)
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Project;
    use rustgis_data::raster::{Crs, GeoTransform, Raster, geotiff};
    use rustgis_data::{Coord, Dataset, Geometry, Value as Cell};
    use rustgis_map::raster::Stretch;

    const COPY: &str = "layer.copySymbology";

    /// A layer of three features, each field filled by its type: 1, 2, 3; 1.5, 3, 4.5; a, b, c.
    fn layer(s: &mut Session, name: &str, geometry: GeometryType, fields: &[(&str, FieldType)]) -> u32 {
        let mut ds = Dataset::new(name, geometry, fields.iter().map(|(n, t)| Field::new(*n, *t)).collect());
        for row in 0..3i64 {
            let x = 10.0 * row as f64;
            let shape = match geometry {
                GeometryType::Point => Some(Geometry::Point(Coord::new(x, 0.0))),
                GeometryType::Polyline => Some(Geometry::Line(vec![vec![Coord::new(x, 0.0), Coord::new(x + 1.0, 1.0)]])),
                GeometryType::Polygon => Some(Geometry::Polygon(vec![vec![vec![
                    Coord::new(x, 0.0),
                    Coord::new(x + 1.0, 0.0),
                    Coord::new(x + 1.0, 1.0),
                    Coord::new(x, 1.0),
                    Coord::new(x, 0.0),
                ]]])),
                GeometryType::None => None,
            };
            let values = fields
                .iter()
                .map(|(_, t)| match t {
                    FieldType::Integer => Cell::Int(row + 1),
                    FieldType::Double => Cell::Float(1.5 * (row + 1) as f64),
                    FieldType::Boolean => Cell::Bool(row == 0),
                    _ => Cell::Text(["a", "b", "c"][row as usize].into()),
                })
                .collect();
            ds.push(shape, values);
        }
        s.map_mut().add_dataset(ds)
    }

    /// A raster layer of `bands` bands, 4 × 4 pixels of `values` over and over (each band one more).
    fn raster(s: &mut Session, name: &str, bands: usize, values: &[f32], color: ColorHint, data_type: &str) -> u32 {
        let gt = GeoTransform { origin_x: 0.0, origin_y: 4.0, pixel_width: 1.0, pixel_height: -1.0 };
        let pixels = (0..bands).map(|b| (0..16).map(|i| values[i % values.len()] + b as f32).collect()).collect();
        let r = Raster::new(name, 4, 4, pixels, gt, Crs::LonLat, None, color, data_type).unwrap();
        let bytes = geotiff::write(&r, 0).unwrap();
        crate::cmd::raster::add_raster(s, name, RasterLayer::from_bytes(r, format!("{name}.tif"), bytes)).unwrap();
        s.map().layer_by_name(name).unwrap().id
    }

    /// What `layer.copySymbology` copies of a feature layer, to compare two layers by.
    fn symbology(s: &Session, id: u32) -> String {
        let l = s.map().layer(id).unwrap();
        format!(
            "{:?}",
            (
                &l.renderer,
                &l.line_strokes,
                &l.fill_layers,
                l.stroke_levels,
                l.no_fill,
                &l.hidden_fills,
                &l.marker_rotation,
                &l.size_by,
                &l.symbol_by,
                &l.diagram,
                l.reference_scale,
                l.transparency
            )
        )
    }

    /// Everything else a layer draws by, which a copy leaves alone.
    fn own(s: &Session, id: u32) -> String {
        let l = s.map().layer(id).unwrap();
        format!(
            "{:?}",
            (&l.name, &l.labels, l.min_scale, l.max_scale, &l.definition_query, l.blend, &l.mask, l.visible, l.dataset.features.len())
        )
    }

    /// A layer that has every field the symbology reads takes all of it, whatever else it holds,
    /// and keeps what isn't symbology; one Undo gives it its own back, and Redo the copy.
    #[test]
    fn a_layer_with_the_fields_takes_the_whole_symbology_and_one_undo_gives_its_own_back() {
        let mut s = Session::new(Project::default());
        let from = layer(&mut s, "Parcels 2020", GeometryType::Polygon, &[("zone", FieldType::Text), ("value", FieldType::Integer)]);
        // The same fields by another case and number type, in another order, with one more.
        let to = layer(
            &mut s,
            "Parcels 2024",
            GeometryType::Polygon,
            &[("note", FieldType::Text), ("VALUE", FieldType::Double), ("Zone", FieldType::Text)],
        );
        s.run("layer.symbology", &json!({"layer": from, "type": "graduated", "field": "value", "classes": 2})).unwrap();
        s.run("layer.symbolBy", &json!({"layer": from, "stroke": "CASE WHEN \"zone\" = 'a' THEN '#ff0000' ELSE '#000000' END"})).unwrap();
        s.run("layer.transparency", &json!({"layer": from, "value": 35})).unwrap();
        s.run(
            "layer.fillLayers",
            &json!({"layer": from, "fills": [{"pattern": "hatch", "color": "#336699", "angle": 45, "spacing": 6, "width": 1}]}),
        )
        .unwrap();
        s.map_mut().layer_mut(from).unwrap().reference_scale = Some(50_000.0);
        // Its own symbology, and what a copy must leave alone.
        s.run("layer.symbology", &json!({"layer": to, "type": "unique", "field": "Zone"})).unwrap();
        s.run("labels.enable", &json!({"layer": to, "on": true})).unwrap();
        s.run("layer.definitionQuery", &json!({"layer": to, "where": "\"VALUE\" > 1"})).unwrap();
        let (before, kept) = (symbology(&s, to), own(&s, to));
        assert_ne!(before, symbology(&s, from));
        s.active_layer = Some(from);

        let mark = s.notice_mark();
        let out = s.run(COPY, &json!({"layer": "Parcels 2024", "from": "Parcels 2020"})).unwrap();
        assert_eq!(out, json!({"layer": "Parcels 2024", "from": "Parcels 2020", "symbology": "Graduated Colors"}));
        assert_eq!(symbology(&s, to), symbology(&s, from), "the whole symbology");
        assert!(!s.map().layer(to).unwrap().fill_layers.is_empty() && s.map().layer(to).unwrap().transparency == 35.0);
        assert_eq!(own(&s, to), kept, "labels, query and the rest stay the layer's own");
        let said: Vec<&str> = s.notices_since(mark).iter().map(|n| n.text.as_str()).collect();
        assert_eq!(said, ["Parcels 2024 is drawn with the symbology of Parcels 2020"], "nothing was left out, so nothing else is said");
        // The layer the panes show stays the one it was.
        assert_eq!(s.active_layer, Some(from));

        s.run("edit.undo", &Value::Null).unwrap();
        assert_eq!(symbology(&s, to), before, "one Undo step");
        assert_eq!(own(&s, to), kept);
        s.run("edit.redo", &Value::Null).unwrap();
        assert_eq!(symbology(&s, to), symbology(&s, from));
        // By id, and to the active layer when none is named.
        s.run("edit.undo", &Value::Null).unwrap();
        s.active_layer = Some(to);
        s.run(COPY, &json!({"from": from})).unwrap();
        assert_eq!(symbology(&s, to), symbology(&s, from));
    }

    /// A layer without what the symbology reads is refused with the reason, which names the
    /// field or type, and is left as it was: nothing is copied in part and no Undo step is made.
    #[test]
    fn a_layer_without_what_the_symbology_reads_is_refused_with_the_reason_and_left_as_it_was() {
        use FieldType::{Double, Integer, Text};
        use GeometryType::{Point, Polygon, Polyline};
        type Fields<'a> = &'a [(&'a str, FieldType)];
        // The layer copied from (A), what styles it, the layer taking it (B), and the reason.
        let cases: Vec<(GeometryType, Fields, Vec<(&str, Value)>, GeometryType, Fields, &str)> = vec![
            (Polygon, &[("pop", Integer)], vec![], Point, &[("pop", Integer)], "A is a polygon layer and B is a point layer"),
            (Polyline, &[("pop", Integer)], vec![], Polygon, &[("pop", Integer)], "A is a polyline layer and B is a polygon layer"),
            (
                Polygon,
                &[("pop", Integer)],
                vec![("layer.symbology", json!({"type": "graduated", "field": "pop", "classes": 2}))],
                Polygon,
                &[("name", Text)],
                "B has no field 'pop' (the Graduated Colors field of A)",
            ),
            (
                Polygon,
                &[("pop", Integer)],
                vec![("layer.symbology", json!({"type": "graduated", "field": "pop", "classes": 2}))],
                Polygon,
                &[("pop", Text)],
                "'pop' is a Text field in B: the Graduated Colors field of A is read as a number",
            ),
            (
                Polygon,
                &[("kind", Integer)],
                vec![("layer.symbology", json!({"type": "unique", "field": "kind"}))],
                Polygon,
                &[("kind", Text)],
                "'kind' is a Text field in B and a Long field in A: the Unique Values field wouldn't match its values",
            ),
            (
                Polygon,
                &[("kind", Text)],
                vec![("layer.symbology", json!({"type": "unique", "field": "kind"}))],
                Polygon,
                &[("type", Text)],
                "B has no field 'kind' (the Unique Values field of A)",
            ),
            (
                Polygon,
                &[("a", Integer), ("b", Integer)],
                vec![("layer.symbology", json!({"type": "unique", "expression": "\"a\" + \"b\""}))],
                Polygon,
                &[("a", Integer)],
                "B has no field 'b' (the Unique Values expression of A)",
            ),
            (
                Polygon,
                &[("pop", Integer)],
                vec![("layer.symbolBy", json!({"fill": "CASE WHEN \"pop\" > 1 THEN '#ff0000' ELSE '#00ff00' END"}))],
                Polygon,
                &[("pop", Text)],
                "'pop' is a Text field in B and a Long field in A: the fill expression wouldn't match its values",
            ),
            (
                Point,
                &[("heading", Double)],
                vec![("layer.markerRotation", json!({"field": "heading"}))],
                Point,
                &[("bearing", Double)],
                "B has no field 'heading' (the marker rotation field of A)",
            ),
            (
                Point,
                &[("kind", Text), ("pop", Integer)],
                vec![
                    ("layer.symbology", json!({"type": "unique", "field": "kind"})),
                    ("layer.sizeBy", json!({"field": "pop", "min": 1, "max": 3})),
                ],
                Point,
                &[("kind", Text)],
                "B has no field 'pop' (the size by field of A)",
            ),
            (
                Point,
                &[("pop", Integer)],
                vec![("layer.symbology", json!({"type": "proportional", "field": "pop"}))],
                Point,
                &[("pop", Text)],
                "'pop' is a Text field in B: the Proportional Symbols field of A is read as a number",
            ),
            (
                Polygon,
                &[("men", Integer), ("women", Integer)],
                vec![("layer.diagram", json!({"kind": "pie", "fields": ["men", "women"], "size": 12}))],
                Polygon,
                &[("men", Integer)],
                "B has no field 'women' (a diagram field of A)",
            ),
        ];
        for (a_shape, a_fields, style, b_shape, b_fields, why) in cases {
            let mut s = Session::new(Project::default());
            let a = layer(&mut s, "A", a_shape, a_fields);
            let b = layer(&mut s, "B", b_shape, b_fields);
            for (command, mut params) in style {
                params["layer"] = json!(a);
                s.run(command, &params).unwrap_or_else(|e| panic!("{command}: {e}"));
            }
            let (before, kept, steps) = (symbology(&s, b), own(&s, b), s.undo_depth());
            assert_eq!(s.run(COPY, &json!({"layer": b, "from": a})).unwrap_err(), why);
            assert_eq!((symbology(&s, b), own(&s, b)), (before, kept), "{why}: nothing copied in part");
            assert_eq!(s.undo_depth(), steps, "{why}: a refusal is no step to undo");
            // The reason the Symbology pane shows on the layer is the command's own.
            let m = s.map();
            assert_eq!(misfit(m.layer(a).unwrap(), m.layer(b).unwrap()).as_deref(), Some(why));
        }
    }

    /// A number field takes classes made from the other number type, as their values print alike;
    /// an expression is read by the fields it names, whatever else the layer copied from holds.
    #[test]
    fn number_types_and_other_fields_do_not_stand_in_the_way() {
        let mut s = Session::new(Project::default());
        let a = layer(
            &mut s,
            "A",
            GeometryType::Point,
            &[("kind", FieldType::Integer), ("spare", FieldType::Text), ("pop", FieldType::Double)],
        );
        let b = layer(&mut s, "B", GeometryType::Point, &[("pop", FieldType::Integer), ("kind", FieldType::Double)]);
        s.run("layer.symbology", &json!({"layer": a, "type": "unique", "field": "kind"})).unwrap();
        s.run("layer.symbolBy", &json!({"layer": a, "size": "\"pop\" * 2 + 4"})).unwrap();
        s.run("layer.markerRotation", &json!({"layer": a, "field": "pop"})).unwrap();
        s.run(COPY, &json!({"layer": b, "from": a})).unwrap();
        assert_eq!(symbology(&s, b), symbology(&s, a));
        // The class of 3 in a Long field is the class of 3.0 in a Double one (B's second row).
        let (from, to) = (s.map().layer(a).unwrap(), s.map().layer(b).unwrap());
        assert_eq!(to.renderer.symbol_for(&to.dataset, 1), from.renderer.symbol_for(&from.dataset, 2));
        assert_ne!(to.renderer.symbol_for(&to.dataset, 1).fill, rustgis_map::symbology::OTHER_FILL);
    }

    /// A setting kept without being drawn is neither needed nor copied: a size by field under a
    /// single symbol, and a marker's size and angle expressions on polygons.
    #[test]
    fn settings_kept_but_not_drawn_are_neither_needed_nor_copied() {
        let mut s = Session::new(Project::default());
        let a = layer(&mut s, "A", GeometryType::Point, &[("kind", FieldType::Text), ("pop", FieldType::Integer)]);
        let b = layer(&mut s, "B", GeometryType::Point, &[("kind", FieldType::Text)]);
        s.run("layer.symbology", &json!({"layer": a, "type": "unique", "field": "kind"})).unwrap();
        s.run("layer.sizeBy", &json!({"layer": a, "field": "pop", "min": 1, "max": 3})).unwrap();
        s.run("layer.symbology", &json!({"layer": a, "type": "single"})).unwrap();
        assert!(s.map().layer(a).unwrap().size_by.is_some(), "kept for when classes come back");
        s.run(COPY, &json!({"layer": b, "from": a})).unwrap();
        assert_eq!(s.map().layer(b).unwrap().size_by, None);
        assert_eq!(s.map().layer(b).unwrap().renderer, s.map().layer(a).unwrap().renderer);

        let c = layer(&mut s, "C", GeometryType::Polygon, &[("pop", FieldType::Integer)]);
        let d = layer(&mut s, "D", GeometryType::Polygon, &[("name", FieldType::Text)]);
        // As a file from elsewhere can hold them: no command sets these on polygons.
        let by = &mut s.map_mut().layer_mut(c).unwrap().symbol_by;
        by.size = Some("\"pop\"".into());
        by.angle = Some("\"pop\"".into());
        s.run(COPY, &json!({"layer": d, "from": c})).unwrap();
        assert!(s.map().layer(d).unwrap().symbol_by.is_empty());
    }

    /// No Fill is copied as it is on the layer copied from: the fills are hidden alike, and
    /// Restore Fill shows the same on both.
    #[test]
    fn no_fill_is_copied_as_it_is_and_restores_alike() {
        let mut s = Session::new(Project::default());
        let a = layer(&mut s, "A", GeometryType::Polygon, &[("kind", FieldType::Text)]);
        let b = layer(&mut s, "B", GeometryType::Polygon, &[("kind", FieldType::Text)]);
        s.run("layer.symbology", &json!({"layer": a, "type": "unique", "field": "kind"})).unwrap();
        let shown = symbology(&s, a);
        s.run("layer.noFill", &json!({"layer": a})).unwrap();
        s.run(COPY, &json!({"layer": b, "from": a})).unwrap();
        assert!(s.map().layer(b).unwrap().no_fill.is_some());
        assert_eq!(symbology(&s, b), symbology(&s, a));
        s.run("layer.restoreFill", &json!({"layer": b})).unwrap();
        assert_eq!(symbology(&s, b), shown, "what No Fill hid comes back on the copy");
    }

    /// Layers that aren't drawn by a renderer of their own, a raster against a feature layer, and
    /// a layer against itself are refused, each with its reason.
    #[test]
    fn other_kinds_of_layer_are_refused() {
        let mut s = Session::new(Project::default());
        let a = layer(&mut s, "A", GeometryType::Polygon, &[("pop", FieldType::Integer)]);
        let table = layer(&mut s, "Rows", GeometryType::None, &[("pop", FieldType::Integer)]);
        let graphics = s.map_mut().add_graphics("Sketch");
        let dem = raster(&mut s, "Relief", 1, &[1.5, 2.5, 3.5], ColorHint::Gray, "Float32");
        for (to, from, why) in [
            (a, table, "Rows is a table: it has no symbols"),
            (table, a, "Rows is a table: it has no symbols"),
            (a, graphics, "Sketch is a graphics layer: each graphic has a style of its own"),
            (graphics, a, "Sketch is a graphics layer: each graphic has a style of its own"),
            (a, dem, "Relief is a raster and A is a feature layer"),
            (dem, a, "A is a feature layer and Relief is a raster"),
            (a, a, "A has this symbology already: name another layer in \"from\""),
        ] {
            assert_eq!(s.run(COPY, &json!({"layer": to, "from": from})).unwrap_err(), why);
        }
    }

    /// A raster takes a symbology whose bands it has, with its stretch, masked values and
    /// transparency, and keeps the angle it is turned by; one Undo gives its own back. One that
    /// hasn't a band, a colour table or whole numbers the symbology needs is refused.
    #[test]
    fn a_raster_takes_a_symbology_whose_bands_it_has() {
        let mut s = Session::new(Project::default());
        let relief = raster(&mut s, "Relief", 1, &[1.5, 2.5, 3.5], ColorHint::Gray, "Float32");
        let image = raster(&mut s, "Image", 3, &[10.0, 20.0, 30.0], ColorHint::Rgb { alpha: false }, "UInt8");
        let style = |s: &Session, id: u32| s.map().layer(id).unwrap().raster.as_ref().unwrap().style.clone();
        s.run("raster.symbology", &json!({"layer": relief, "type": "stretch", "ramp": "viridis", "invert": true})).unwrap();
        s.run("raster.stretch", &json!({"layer": relief, "type": "custom", "min": 1, "max": 3})).unwrap();
        s.run("raster.mask", &json!({"layer": relief, "values": [2.5]})).unwrap();
        s.run("layer.transparency", &json!({"layer": relief, "value": 20})).unwrap();
        s.map_mut().layer_mut(image).unwrap().raster.as_mut().unwrap().style.rotation = 30.0;
        let before = style(&s, image);
        assert!(matches!(before.renderer, RasterRenderer::Rgb { .. }));

        let out = s.run(COPY, &json!({"layer": "Image", "from": "Relief"})).unwrap();
        assert_eq!(out["symbology"], "Stretch");
        let (now, like) = (style(&s, image), style(&s, relief));
        assert_eq!(now.rotation, 30.0, "where it lies stays its own");
        assert_eq!(RasterStyle { rotation: like.rotation, ..now }, like, "the rest is the symbology copied");
        assert_eq!(like.stretch, Stretch::custom(1.0, 3.0));
        assert_eq!(s.map().layer(image).unwrap().transparency, 20.0);
        s.run("edit.undo", &Value::Null).unwrap();
        assert_eq!(style(&s, image), before, "one Undo step");
        assert_eq!(s.map().layer(image).unwrap().transparency, 0.0);

        // Three bands as red, green and blue need three bands.
        assert_eq!(s.run(COPY, &json!({"layer": relief, "from": image})).unwrap_err(), "Image draws band 2 and Relief has 1 band");
        // A colour table is the raster's own.
        let mut palette = vec![[0, 0, 0, 255]; 256];
        palette[1] = [200, 30, 30, 255];
        let cover = raster(&mut s, "Cover", 1, &[1.0, 2.0, 3.0], ColorHint::Palette(palette.clone()), "UInt8");
        let cover_2 = raster(&mut s, "Cover 2", 1, &[3.0, 2.0], ColorHint::Palette(palette), "UInt8");
        s.run("raster.symbology", &json!({"layer": cover, "type": "colormap"})).unwrap();
        s.run("raster.symbology", &json!({"layer": cover_2, "type": "stretch"})).unwrap();
        assert_eq!(
            s.run(COPY, &json!({"layer": relief, "from": cover})).unwrap_err(),
            "Cover draws its own colour table and Relief has none"
        );
        s.run(COPY, &json!({"layer": cover_2, "from": cover})).unwrap();
        assert_eq!(style(&s, cover_2).renderer, RasterRenderer::Colormap);
        // Unique Values colour whole numbers.
        s.run("raster.symbology", &json!({"layer": cover, "type": "unique"})).unwrap();
        assert_eq!(
            s.run(COPY, &json!({"layer": relief, "from": cover})).unwrap_err(),
            "Cover draws Unique Values and band 1 of Relief isn't whole numbers with at most 256 distinct values"
        );
        s.run(COPY, &json!({"layer": cover_2, "from": cover})).unwrap();
        assert_eq!(style(&s, cover_2).renderer, style(&s, cover).renderer);
        // Pixels that aren't read can't be checked.
        s.map_mut().layer_mut(cover_2).unwrap().raster.as_mut().unwrap().data = None;
        assert_eq!(
            s.run(COPY, &json!({"layer": cover_2, "from": relief})).unwrap_err(),
            "Cover 2's pixels aren't read, so its bands can't be checked"
        );
        // The symbology of one whose pixels aren't read is still its own to give.
        s.run(COPY, &json!({"layer": relief, "from": cover_2})).unwrap_err();
        s.run("raster.symbology", &json!({"layer": relief, "type": "stretch"})).unwrap();
        let unread = raster(&mut s, "Unread", 1, &[1.5, 2.5], ColorHint::Gray, "Float32");
        s.run("raster.symbology", &json!({"layer": unread, "type": "classify", "classes": 2})).unwrap();
        s.map_mut().layer_mut(unread).unwrap().raster.as_mut().unwrap().data = None;
        s.run(COPY, &json!({"layer": relief, "from": unread})).unwrap();
        assert!(matches!(style(&s, relief).renderer, RasterRenderer::Classify { .. }));
    }

    /// Junk parameters are refused with what is wrong, never a panic, and change nothing.
    #[test]
    fn junk_parameters_are_refused_without_a_change() {
        let mut s = Session::new(Project::default());
        let a = layer(&mut s, "A", GeometryType::Polygon, &[("pop", FieldType::Integer)]);
        let b = layer(&mut s, "B", GeometryType::Polygon, &[("pop", FieldType::Integer)]);
        s.run("layer.symbology", &json!({"layer": a, "type": "graduated", "field": "pop", "classes": 2})).unwrap();
        s.active_layer = Some(b);
        let (before, steps) = (symbology(&s, b), s.undo_depth());
        let required = "from is required: the name or id of the layer whose symbology to copy";
        for (params, why) in [
            (Value::Null, required),
            (json!({}), required),
            (json!({"layer": b}), required),
            (json!({"layer": b, "from": null}), required),
            (json!({"layer": b, "from": ""}), required),
            (json!({"layer": b, "from": "   "}), required),
            (json!({"layer": b, "from": ["A"]}), required),
            (json!({"layer": b, "from": {"name": "A"}}), required),
            (json!({"layer": b, "from": true}), required),
            (json!({"layer": b, "from": "Nope"}), "no layer named 'Nope'"),
            (json!({"layer": b, "from": 99999}), "no layer with id 99999"),
            (json!({"layer": b, "from": -1}), "no layer with id -1"),
            (json!({"layer": b, "from": 1e300}), "no layer with id 1e+300"),
            (json!({"layer": "Nope", "from": a}), "no layer named 'Nope'"),
            (json!({"layer": 1e300, "from": a}), "no layer with id 1e+300"),
        ] {
            assert_eq!(s.run(COPY, &params).unwrap_err(), why, "{params}");
        }
        assert_eq!((symbology(&s, b), s.undo_depth()), (before, steps));
        // What isn't a selector is ignored, as other commands ignore keys they don't read.
        s.run(COPY, &json!({"layer": [b], "from": a, "labels": "x"})).unwrap();
    }

    /// Both layers are bound as the command's inputs, for a reviewed run (the Assistant's, a
    /// plugin's): the one styled and the one copied from, by name or id; a name the map hasn't
    /// yet is one an earlier step makes.
    #[test]
    fn both_layers_are_bound_as_inputs() {
        use crate::cmd::InputSource;
        let mut s = Session::new(Project::default());
        let a = layer(&mut s, "A", GeometryType::Polygon, &[("pop", FieldType::Integer)]);
        let b = layer(&mut s, "B", GeometryType::Polygon, &[("pop", FieldType::Integer)]);
        let map = s.map().instance_id();
        let bound = |p: Value| copy_inputs(&s.input_context(), &p).map(|i| (i.layers, i.future_layers, i.uses_default));
        assert_eq!(bound(json!({"layer": "B", "from": "A"})).unwrap(), (vec![(map, b), (map, a)], vec![], false));
        assert_eq!(bound(json!({"layer": b, "from": a})).unwrap(), (vec![(map, b), (map, a)], vec![], false));
        // The active layer by default: bound when the step runs.
        assert_eq!(bound(json!({"from": "A"})).unwrap(), (vec![(map, a)], vec![], true));
        assert_eq!(bound(json!({"layer": "B", "from": "Made Later"})).unwrap(), (vec![(map, b)], vec!["Made Later".to_string()], false));
        assert!(bound(json!({"layer": "B"})).is_err());
        assert!(bound(json!({"layer": "B", "from": 99999})).is_err());
    }
}
