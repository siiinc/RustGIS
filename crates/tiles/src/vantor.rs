//! Vantor's Transform API and Raster Analytics: a satellite collect made into imagery on request
//! (orthorectified, pan-sharpened, NDVI…) and answered as a cloud-optimised GeoTIFF, which is
//! read in place by its address. This module lists what the two services offer ([`SERVICES`]:
//! their scripts, each script's functions and each function's parameters) and builds a request's
//! address ([`address`]). It does no I/O.
//!
//! An address carries no credential. The Transform API takes a Bearer token in the
//! `Authorization` header and Raster Analytics an API key in the `maxar_api_key` query parameter:
//! a saved connection's login sends either ([`Service::auth`], [`Service::key_name`]), only to the
//! service's own origin, and keeps it out of every address that is saved or shown.
//!
//! The services' ideal (sensor geometry) imagery is left out: it has no coordinate system to
//! place it on a map with.

use crate::s3::encode;

/// What a parameter's value is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Text, sent quoted.
    Text,
    /// `true` or `false`.
    Bool,
    /// A number.
    Number,
}

/// A parameter of a function.
#[derive(Clone, Copy, Debug)]
pub struct Param {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub help: &'static str,
    /// The values offered for it; any other may still be typed where a service takes more.
    pub choices: &'static [&'static str],
}

/// A function of a script: one kind of imagery.
#[derive(Clone, Copy, Debug)]
pub struct Function {
    pub name: &'static str,
    pub summary: &'static str,
    pub params: &'static [Param],
}

/// A script of a service: an endpoint with its functions.
#[derive(Clone, Copy, Debug)]
pub struct Script {
    pub name: &'static str,
    pub functions: &'static [Function],
}

/// One of Vantor's services that answer with GeoTIFFs made on request.
#[derive(Clone, Copy, Debug)]
pub struct Service {
    pub id: &'static str,
    pub label: &'static str,
    /// Where the service is, unless another address is given.
    pub address: &'static str,
    /// How a saved connection logs in to it: `header` or `query` (a web-service connection's
    /// `auth`).
    pub auth: &'static str,
    /// The header or query parameter its credential goes in.
    pub key_name: &'static str,
    pub scripts: &'static [Script],
}

impl Service {
    pub fn script(&self, name: &str) -> Option<&'static Script> {
        self.scripts.iter().find(|s| s.name == name)
    }
}

impl Script {
    pub fn function(&self, name: &str) -> Option<&'static Function> {
        self.functions.iter().find(|f| f.name == name)
    }
}

/// The service with this id.
pub fn service(id: &str) -> Option<&'static Service> {
    SERVICES.iter().find(|s| s.id == id)
}

const fn text(name: &'static str, required: bool, help: &'static str) -> Param {
    Param { name, kind: Kind::Text, required, help, choices: &[] }
}

const fn flag(name: &'static str, help: &'static str) -> Param {
    Param { name, kind: Kind::Bool, required: false, help, choices: &[] }
}

const fn number(name: &'static str, help: &'static str) -> Param {
    Param { name, kind: Kind::Number, required: false, help, choices: &[] }
}

const COLLECT: Param = text("collect_identifier", true, "The collect's identifier.");
const CRS: Param = text("crs", true, "The image's coordinate system: UTM, or a code such as EPSG:3857.");
const BANDS: Param = text("bands", false, "The bands, comma-separated: red,green,blue unless given; pan for the panchromatic band.");
const BANDS_MS: Param = text("bands", false, "The multispectral bands, comma-separated: red,green,blue unless given (not pan).");
const INTERPOLATION: Param = Param {
    name: "interpolation",
    kind: Kind::Text,
    required: false,
    help: "How the pixels are resampled.",
    choices: &["NearestNeighbor", "Bilinear", "Bicubic", "MTF"],
};
const DRA: Param = flag("dra", "8-bit imagery with its dynamic range adjusted (on unless given); off answers 16-bit values.");
const ACOMP: Param = flag("acomp", "Atmospheric compensation (off unless given).");
const HD: Param = flag("hd", "A higher resolution than the collect's own (off unless given).");
const GSD: Param = number("gsd", "The pixel size in metres (the collect's own unless given).");
const ANCILLARY: Param = text("ancillary_type", false, "The ancillary data used (the best there is unless given).");
const DEM: Param = text("dem_name", false, "The elevation model used, such as 4m-p3d-dtm.");

const ORTHO_SUMMARY: &str = "An orthorectified image.";
const PANSHARP_SUMMARY: &str = "An orthorectified, pan-sharpened image.";

const ORTHO_WITH_OVERVIEWS: Script = Script {
    name: "ortho-with-overviews",
    functions: &[
        Function { name: "ortho", summary: ORTHO_SUMMARY, params: &[COLLECT, CRS, BANDS, INTERPOLATION, DRA, ACOMP, HD] },
        Function { name: "pansharp_ortho", summary: PANSHARP_SUMMARY, params: &[COLLECT, CRS, BANDS_MS, DRA, ACOMP, HD] },
    ],
};

const ORTHO: Script = Script {
    name: "ortho",
    functions: &[
        Function { name: "ortho", summary: ORTHO_SUMMARY, params: &[COLLECT, CRS, BANDS, INTERPOLATION, DRA, ACOMP, HD, GSD] },
        Function { name: "pansharp_ortho", summary: PANSHARP_SUMMARY, params: &[COLLECT, CRS, BANDS_MS, DRA, ACOMP, HD, GSD] },
    ],
};

const ORTHO_MGP: Script = Script {
    name: "ortho-with-overviews-mgp-image-server-v1",
    functions: &[
        Function { name: "ortho", summary: ORTHO_SUMMARY, params: &[COLLECT, CRS, BANDS, INTERPOLATION, DRA, ACOMP, HD, ANCILLARY, DEM] },
        Function { name: "pansharp_ortho", summary: PANSHARP_SUMMARY, params: &[COLLECT, CRS, BANDS_MS, DRA, ACOMP, HD, ANCILLARY, DEM] },
    ],
};

const BROWSE: Script =
    Script { name: "browse", functions: &[Function { name: "browse", summary: "A browse image.", params: &[COLLECT, CRS, BANDS] }] };

const NDVI: Script = Script {
    name: "ndvi-image",
    functions: &[
        Function { name: "ndvi", summary: "NDVI as floating-point values.", params: &[COLLECT, CRS] },
        Function { name: "ndvi_dra", summary: "NDVI as an 8-bit grey image.", params: &[COLLECT, CRS] },
        Function { name: "ndvi_colorized", summary: "NDVI as an 8-bit colour image.", params: &[COLLECT, CRS] },
    ],
};

/// The Transform API: its credential is a Bearer token.
pub const TRANSFORM: Service = Service {
    id: "transform",
    label: "Transform API",
    address: "https://api.transform.satcloud.us/v1/ipe",
    auth: "header",
    key_name: "Authorization",
    scripts: &[ORTHO_WITH_OVERVIEWS, ORTHO, ORTHO_MGP, BROWSE],
};

/// Raster Analytics: its credential is an API key.
pub const RASTER_ANALYTICS: Service = Service {
    id: "rasterAnalytics",
    label: "Raster Analytics",
    address: "https://api.maxar.com/analytics/v1/raster",
    auth: "query",
    key_name: "maxar_api_key",
    scripts: &[ORTHO_WITH_OVERVIEWS, ORTHO, NDVI, BROWSE],
};

/// The services, the Transform API first.
pub const SERVICES: &[Service] = &[TRANSFORM, RASTER_ANALYTICS];

/// The longest address of a service, and the longest value of a parameter.
const MAX_ADDRESS: usize = 2048;
const MAX_VALUE: usize = 512;

/// A service's address as a request is built on: `http://` or `https://`, with no query,
/// fragment, user information, spaces or control characters, and no slash at its end.
pub fn service_address(address: &str) -> Result<&str, String> {
    let a = address.trim().trim_end_matches('/');
    let lower = a.to_ascii_lowercase();
    let rest = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://"));
    let plain = !a.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '?' | '#' | '@' | '\\'));
    match rest {
        Some(rest) if !rest.is_empty() && !rest.starts_with('/') && plain && a.len() <= MAX_ADDRESS => Ok(a),
        _ => Err("Give the service's address: https://… with no query, such as https://api.transform.satcloud.us/v1/ipe".into()),
    }
}

/// The address of the GeoTIFF `function` of `script` makes at the service at `service`, from the
/// values given for its parameters (by name; one left empty isn't sent). Text goes quoted, as the
/// services read it: `…/ortho-with-overviews/geotiff?function=pansharp_ortho&p=crs=%22EPSG%3A3857%22`.
/// The address holds no credential.
pub fn address(service: &str, script: &Script, function: &Function, values: &[(&str, &str)]) -> Result<String, String> {
    let service = service_address(service)?;
    let mut out = format!("{service}/{}/geotiff?function={}", script.name, function.name);
    for p in function.params {
        let value = values.iter().find(|(name, _)| *name == p.name).map_or("", |(_, v)| v.trim());
        if value.is_empty() {
            if p.required {
                return Err(format!("{} is required", p.name));
            }
            continue;
        }
        if value.len() > MAX_VALUE || value.chars().any(|c| c.is_control() || c == '"') {
            return Err(format!("{} can't hold a quote or a control character, and is at most {MAX_VALUE} characters", p.name));
        }
        let sent = match p.kind {
            Kind::Text => {
                if p.name == "crs" && !is_crs(value) {
                    return Err("crs is UTM or a code such as EPSG:3857".into());
                }
                encode(&format!("\"{value}\""), true)
            }
            Kind::Bool => match value {
                "true" | "false" => value.to_string(),
                _ => return Err(format!("{} is true or false", p.name)),
            },
            Kind::Number => match value.parse::<f64>() {
                Ok(n) if n.is_finite() => encode(value, true),
                _ => return Err(format!("{} is a number", p.name)),
            },
        };
        out.push_str(&format!("&p={}={sent}", p.name));
    }
    Ok(out)
}

/// Whether `value` names a coordinate system the services take: `UTM`, or `EPSG:` and a code.
fn is_crs(value: &str) -> bool {
    let v = value.to_ascii_uppercase();
    v == "UTM"
        || v.strip_prefix("EPSG:").is_some_and(|code| !code.is_empty() && code.len() <= 6 && code.bytes().all(|b| b.is_ascii_digit()))
}

/// The `Authorization` header's value for a token of the Transform API: `Bearer` and the token,
/// which is one word of JWT characters (a `Bearer ` pasted before it is dropped). Anything else is
/// refused, so it can't break the header it goes in.
pub fn bearer(token: &str) -> Result<String, String> {
    let token = token.trim();
    let token = token.get(..7).filter(|start| start.eq_ignore_ascii_case("bearer ")).map_or(token, |_| token.get(7..).unwrap_or("").trim());
    let word = !token.is_empty()
        && !token.eq_ignore_ascii_case("bearer")
        && token.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.~+/=".contains(&b));
    // A login's value is at most 4096 bytes, `Bearer ` included.
    if !word || token.len() > 4096 - 7 {
        return Err("A Bearer token is one word of letters, digits and - _ . ~ + / =".into());
    }
    Ok(format!("Bearer {token}"))
}

/// When a Bearer token expires (Unix seconds), as its JWT `exp` claim says, if it says.
pub fn token_expiry(token: &str) -> Option<i64> {
    let header = bearer(token).ok()?;
    crate::earthdata::jwt_expiry(header.strip_prefix("Bearer ")?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(service: &str, script: &str, function: &str) -> (&'static Script, &'static Function) {
        let script = super::service(service).and_then(|s| s.script(script)).unwrap();
        (script, script.function(function).unwrap())
    }

    #[test]
    fn an_address_is_built_from_a_function_and_its_values() {
        let (script, function) = pick("rasterAnalytics", "ortho-with-overviews", "pansharp_ortho");
        let values = [
            ("collect_identifier", " B040001101029A10 "),
            ("crs", "EPSG:3857"),
            ("bands", "red,green,blue"),
            ("dra", "true"),
            ("acomp", "false"),
            ("hd", "false"),
            ("interpolation", ""),
            ("unknown", "left out"),
        ];
        assert_eq!(
            address("https://api.maxar.com/analytics/v1/raster/", script, function, &values).unwrap(),
            "https://api.maxar.com/analytics/v1/raster/ortho-with-overviews/geotiff?function=pansharp_ortho\
             &p=collect_identifier=%22B040001101029A10%22&p=crs=%22EPSG%3A3857%22&p=bands=%22red%2Cgreen%2Cblue%22\
             &p=dra=true&p=acomp=false&p=hd=false"
        );
        // A number goes as it is typed, and the parameters in the function's order.
        let (script, function) = pick("transform", "ortho", "ortho");
        let url = address(
            "https://api.transform.satcloud.us/v1/ipe",
            script,
            function,
            &[("gsd", "0.5"), ("crs", "utm"), ("bands", "  "), ("collect_identifier", "X")],
        )
        .unwrap();
        assert!(url.ends_with("/ortho/geotiff?function=ortho&p=collect_identifier=%22X%22&p=crs=%22utm%22&p=gsd=0.5"), "{url}");
    }

    #[test]
    fn values_that_can_not_be_sent_are_refused() {
        let (script, function) = pick("transform", "ortho", "ortho");
        let base = "https://api.transform.satcloud.us/v1/ipe";
        let ok = [("collect_identifier", "X"), ("crs", "EPSG:4326")];
        assert!(address(base, script, function, &ok).is_ok());
        for (values, said) in [
            (vec![("crs", "EPSG:4326")], "collect_identifier is required"),
            (vec![("collect_identifier", "X")], "crs is required"),
            (vec![("collect_identifier", "X"), ("crs", "Mercator")], "crs is UTM"),
            (vec![("collect_identifier", "X"), ("crs", "EPSG:")], "crs is UTM"),
            (vec![("collect_identifier", "X\"&function=y"), ("crs", "UTM")], "can't hold a quote"),
            (vec![("collect_identifier", "X\n"), ("crs", "UTM"), ("bands", "a\tb")], "can't hold a quote"),
            (vec![("collect_identifier", "X"), ("crs", "UTM"), ("dra", "yes")], "dra is true or false"),
            (vec![("collect_identifier", "X"), ("crs", "UTM"), ("gsd", "NaN")], "gsd is a number"),
            (vec![("collect_identifier", "X"), ("crs", "UTM"), ("gsd", "1&p=x")], "gsd is a number"),
        ] {
            let e = address(base, script, function, &values).unwrap_err();
            assert!(e.contains(said), "{values:?}: {e}");
        }
        // What a value holds stays inside its parameter.
        let url = address(base, script, function, &[("collect_identifier", "a&b=c#d"), ("crs", "UTM")]).unwrap();
        assert!(url.contains("p=collect_identifier=%22a%26b%3Dc%23d%22&p=crs="), "{url}");
        for bad in
            ["", "api.example.com", "ftp://x.org/a", "https://", "https://x.org/a?key=1", "https://user@x.org/a", "https://x.org/a b"]
        {
            assert!(address(bad, script, function, &ok).is_err(), "{bad}");
        }
        assert_eq!(service_address(" http://localhost:8080/v1/ipe// ").unwrap(), "http://localhost:8080/v1/ipe");
    }

    #[test]
    fn the_services_and_their_logins() {
        let transform = service("transform").unwrap();
        assert_eq!((transform.auth, transform.key_name), ("header", "Authorization"));
        let analytics = service("rasterAnalytics").unwrap();
        assert_eq!((analytics.auth, analytics.key_name), ("query", "maxar_api_key"));
        assert!(crate::logins::valid_header_name(transform.key_name) && crate::logins::valid_query_name(analytics.key_name));
        assert!(service("other").is_none() && transform.script("ndvi-image").is_none() && analytics.script("ndvi-image").is_some());
        // Every function names its collect first, and no parameter twice.
        for s in SERVICES {
            assert!(service_address(s.address).is_ok(), "{}", s.id);
            for script in s.scripts {
                for f in script.functions {
                    assert_eq!(f.params.first().map(|p| p.name), Some("collect_identifier"), "{}/{}", script.name, f.name);
                    let mut names: Vec<_> = f.params.iter().map(|p| p.name).collect();
                    names.sort_unstable();
                    names.dedup();
                    assert_eq!(names.len(), f.params.len(), "{}/{}", script.name, f.name);
                }
            }
        }
    }

    #[test]
    fn a_bearer_token_is_one_word() {
        assert_eq!(bearer(" abc.DEF-1_2 ").unwrap(), "Bearer abc.DEF-1_2");
        assert_eq!(bearer("Bearer abc").unwrap(), "Bearer abc");
        assert_eq!(bearer("bearer  abc").unwrap(), "Bearer abc");
        for bad in ["", "Bearer ", "two words", "line\nbreak", "quote\"", "é"] {
            assert!(bearer(bad).is_err(), "{bad:?}");
        }
        assert!(bearer(&"a".repeat(4096 - 7)).is_ok() && bearer(&"a".repeat(4096 - 6)).is_err());
        // {"exp": 1893456000} as a JWT's middle part.
        let jwt = "e30.eyJleHAiOjE4OTM0NTYwMDB9.sig";
        assert_eq!(token_expiry(jwt), Some(1_893_456_000));
        assert_eq!(token_expiry(&format!("Bearer {jwt}")), Some(1_893_456_000));
        assert_eq!(token_expiry("not-a-jwt"), None);
    }
}
