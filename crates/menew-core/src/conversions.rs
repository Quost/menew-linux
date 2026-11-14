//! Unit conversions (port of the "Unidades" part of `Calculator.swift`): "5 km em mi", "72f to c".

use crate::calculator::{number, CalcResult};
use crate::format::format_number;
use crate::i18n::Language;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dim {
    Length,
    Mass,
    Volume,
    Data,
    Time,
    Speed,
    Temperature,
    Area,
}

/// (names, dimension, factor to the base unit: metre, gram, litre, byte, second, m/s, m²)
const UNITS: &[(&[&str], Dim, f64)] = &[
    (&["mm", "milimetro", "milimetros"], Dim::Length, 0.001),
    (&["cm", "centimetro", "centimetros"], Dim::Length, 0.01),
    (&["m", "metro", "metros", "meter"], Dim::Length, 1.0),
    (&["km", "quilometro", "quilometros", "kilometer"], Dim::Length, 1000.0),
    (&["in", "pol", "polegada", "polegadas", "inch", "inches", "\""], Dim::Length, 0.0254),
    (&["ft", "pe", "pes", "feet", "foot"], Dim::Length, 0.3048),
    (&["yd", "jarda", "jardas", "yard", "yards"], Dim::Length, 0.9144),
    (&["mi", "milha", "milhas", "mile", "miles"], Dim::Length, 1609.344),
    (&["mg"], Dim::Mass, 0.001),
    (&["g", "grama", "gramas", "gram"], Dim::Mass, 1.0),
    (&["kg", "quilo", "quilos", "kilo"], Dim::Mass, 1000.0),
    (&["t", "ton", "tonelada", "toneladas"], Dim::Mass, 1_000_000.0),
    (&["oz", "onca", "ounce", "ounces"], Dim::Mass, 28.349523125),
    (&["lb", "lbs", "libra", "libras", "pound", "pounds"], Dim::Mass, 453.59237),
    (&["ml"], Dim::Volume, 0.001),
    (&["l", "litro", "litros", "liter", "liters"], Dim::Volume, 1.0),
    (&["gal", "galao", "galoes", "gallon", "gallons"], Dim::Volume, 3.785411784),
    (&["floz"], Dim::Volume, 0.0295735295625),
    (&["xicara", "xicaras", "cup", "cups"], Dim::Volume, 0.24),
    (&["b", "byte", "bytes"], Dim::Data, 1.0),
    (&["kb"], Dim::Data, 1e3),
    (&["mb"], Dim::Data, 1e6),
    (&["gb"], Dim::Data, 1e9),
    (&["tb"], Dim::Data, 1e12),
    (&["kib"], Dim::Data, 1024.0),
    (&["mib"], Dim::Data, 1_048_576.0),
    (&["gib"], Dim::Data, 1_073_741_824.0),
    (&["s", "seg", "segundo", "segundos", "sec", "second", "seconds"], Dim::Time, 1.0),
    (&["min", "minuto", "minutos", "minute", "minutes"], Dim::Time, 60.0),
    (&["h", "hora", "horas", "hour", "hours"], Dim::Time, 3600.0),
    (&["d", "dia", "dias", "day", "days"], Dim::Time, 86400.0),
    (&["sem", "semana", "semanas", "week", "weeks"], Dim::Time, 604_800.0),
    (&["km/h", "kmh"], Dim::Speed, 1.0 / 3.6),
    (&["m/s"], Dim::Speed, 1.0),
    (&["mph"], Dim::Speed, 0.44704),
    (&["kn", "no", "nos", "knot", "knots"], Dim::Speed, 0.514444),
    (&["m2", "m²"], Dim::Area, 1.0),
    (&["km2", "km²"], Dim::Area, 1e6),
    (&["ha", "hectare", "hectares"], Dim::Area, 10_000.0),
    (&["ft2", "ft²"], Dim::Area, 0.09290304),
    (&["acre", "acres"], Dim::Area, 4046.8564224),
    (&["c", "°c", "celsius"], Dim::Temperature, 0.0),
    (&["f", "°f", "fahrenheit"], Dim::Temperature, 0.0),
    (&["k", "kelvin"], Dim::Temperature, 0.0),
];

const CONNECTORS: &[&str] = &["em", "para", "in", "to", "nach", "en", "a", "->", "→", "=", "转"];

struct Unit {
    dim: Dim,
    factor: f64,
    /// First name of the entry, the canonical one.
    canonical: &'static str,
}

fn unit(name: &str) -> Option<Unit> {
    UNITS
        .iter()
        .find(|(names, _, _)| names.contains(&name))
        .map(|(names, dim, factor)| Unit { dim: *dim, factor: *factor, canonical: names[0] })
}

fn symbol(u: &Unit) -> String {
    match u.dim {
        Dim::Temperature => format!("°{}", u.canonical.to_uppercase()),
        Dim::Data => {
            if u.canonical.chars().count() == 3 {
                let first: String = u.canonical.chars().take(1).collect::<String>().to_uppercase();
                let last: String = u.canonical.chars().last().map(|c| c.to_string()).unwrap_or_default().to_uppercase();
                format!("{}i{}", first, last)
            } else {
                u.canonical.to_uppercase()
            }
        }
        _ => u.canonical.to_string(),
    }
}

/// `(value, from unit, to unit)` of "5 km em mi" or "5km mi".
pub fn split_conversion(q: &str) -> Option<(f64, String, String)> {
    let mut words: Vec<String> = q.split(' ').filter(|s| !s.is_empty()).map(String::from).collect();
    // number glued to the unit: "5km"
    if let Some(first) = words.first().cloned() {
        if let Some((i, _)) = first.char_indices().find(|(_, c)| !(c.is_numeric() || *c == ',' || *c == '.')) {
            if i != 0 {
                words.splice(0..1, [first[..i].to_string(), first[i..].to_string()]);
            }
        }
    }
    // the connector sits between the units ("10 cm in in": the second "in" is the unit)
    if words.len() == 4 && CONNECTORS.contains(&words[2].as_str()) {
        words.remove(2);
    }
    if words.len() != 3 {
        return None;
    }
    let v = number(&words[0])?;
    Some((v, words[1].clone(), words[2].clone()))
}

/// "5 km em mi", "72f to c", "1,5 gb em mb"
pub fn convert(q: &str, lang: Language) -> Option<CalcResult> {
    let (value, from, to) = split_conversion(q)?;
    let f = unit(&from)?;
    let t = unit(&to)?;
    if f.dim != t.dim {
        return None;
    }
    let out = if f.dim == Dim::Temperature {
        let celsius = match f.canonical {
            "f" => (value - 32.0) * 5.0 / 9.0,
            "k" => value - 273.15,
            _ => value,
        };
        match t.canonical {
            "f" => celsius * 9.0 / 5.0 + 32.0,
            "k" => celsius + 273.15,
            _ => celsius,
        }
    } else {
        value * f.factor / t.factor
    };
    Some(CalcResult {
        value: format!("{} {}", format_number(out, 4, lang), symbol(&t)),
        detail: format!("{} {}", format_number(value, 10, lang), symbol(&f)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(q: &str, lang: Language) -> Option<(String, String)> {
        convert(q, lang).map(|r| (r.value, r.detail))
    }

    // The vectors V007 to V012 of docs/palette-spec.md
    #[test]
    fn spec_vectors() {
        let en = Language::En;
        assert_eq!(c("5 km to mi", en), Some(("3.1069 mi".into(), "5 km".into())));
        assert_eq!(c("5 km em mi", Language::Pt), Some(("3,1069 mi".into(), "5 km".into())));
        assert_eq!(c("72 f to c", en), Some(("22.2222 °C".into(), "72 °F".into())));
        assert_eq!(c("72f to c", en), Some(("22.2222 °C".into(), "72 °F".into())));
        assert_eq!(c("1,5 gb em mb", Language::Pt), Some(("1500 MB".into(), "1,5 GB".into())));
        assert_eq!(c("10 cm in in", en), Some(("3.937 in".into(), "10 cm".into())));
    }

    #[test]
    fn temperatures_and_data_symbols() {
        let en = Language::En;
        assert_eq!(c("0 c to k", en).unwrap().0, "273.15 °K");
        assert_eq!(c("100 c to f", en).unwrap().0, "212 °F");
        assert_eq!(c("1 gib to mib", en).unwrap().0, "1024 MiB");
        assert_eq!(c("2 kb to b", en).unwrap().0, "2000 B");
        assert_eq!(c("1 h to min", en).unwrap().0, "60 min");
        assert_eq!(c("90 km/h to m/s", en).unwrap().0, "25 m/s");
    }

    #[test]
    fn different_dimensions_and_unknown_units_do_not_convert() {
        assert_eq!(c("5 km to kg", Language::En), None);
        assert_eq!(c("5 foo to bar", Language::En), None);
        assert_eq!(c("km to mi", Language::En), None);
    }

    #[test]
    fn splitting() {
        assert_eq!(split_conversion("5km mi"), Some((5.0, "km".into(), "mi".into())));
        assert_eq!(split_conversion("5 km to mi"), Some((5.0, "km".into(), "mi".into())));
        assert_eq!(split_conversion("1,5 gb em mb"), Some((1.5, "gb".into(), "mb".into())));
        assert_eq!(split_conversion("5 km"), None);
    }
}
