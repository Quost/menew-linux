//! Locale-aware formatting of numbers, times, dates and currencies for the 5 languages. The Mac app
//! gets this from Foundation; here the rules are written out for the languages Menew supports.

use crate::i18n::Language;
use chrono::{Datelike, NaiveDateTime, Timelike};

/// Decimal separator: "," in Portuguese, German and Spanish.
pub fn decimal_sep(lang: Language) -> char {
    match lang {
        Language::Pt | Language::De | Language::Es => ',',
        _ => '.',
    }
}

/// C's `%.6g`, used for huge and tiny values.
fn format_g6(v: f64) -> String {
    let mut exp = v.abs().log10().floor() as i32;
    let mut mant = v / 10f64.powi(exp);
    // rounding to 5 decimals can reach 10.00000
    if (mant.abs() * 1e5).round() / 1e5 >= 10.0 {
        mant /= 10.0;
        exp += 1;
    }
    let m = format!("{:.5}", mant);
    let m = m.trim_end_matches('0').trim_end_matches('.');
    format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
}

/// Up to `digits` decimals, no trailing zeros, no thousands separator (easy to paste anywhere).
/// Port of `Calculator.format`.
pub fn format_number(v: f64, digits: usize, lang: Language) -> String {
    if v != 0.0 && (v.abs() >= 1e15 || v.abs() < 1e-9) {
        return format_g6(v);
    }
    let mut s = format!("{:.*}", digits, v);
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    if s == "-0" {
        s = "0".into();
    }
    if decimal_sep(lang) != '.' {
        s = s.replace('.', ",");
    }
    s
}

/// "3:00 PM" in English, "15:00" elsewhere.
pub fn format_time(h: u32, m: u32, lang: Language) -> String {
    if lang == Language::En {
        let h12 = match h % 12 {
            0 => 12,
            x => x,
        };
        format!("{}:{:02} {}", h12, m, if h >= 12 { "PM" } else { "AM" })
    } else {
        format!("{:02}:{:02}", h, m)
    }
}

pub fn format_time_secs(h: u32, m: u32, s: u32, lang: Language) -> String {
    if lang == Language::En {
        let h12 = match h % 12 {
            0 => 12,
            x => x,
        };
        format!("{}:{:02}:{:02} {}", h12, m, s, if h >= 12 { "PM" } else { "AM" })
    } else {
        format!("{:02}:{:02}:{:02}", h, m, s)
    }
}

const MONTHS_EN: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MONTHS_PT: [&str; 12] = ["jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.", "dez."];
const MONTHS_ES: [&str; 12] = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"];
const WEEKDAYS_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const WEEKDAYS_PT: [&str; 7] = ["seg.", "ter.", "qua.", "qui.", "sex.", "sáb.", "dom."];
const WEEKDAYS_DE: [&str; 7] = ["Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa.", "So."];
const WEEKDAYS_ES: [&str; 7] = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
const WEEKDAYS_ZH: [&str; 7] = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];

/// Abbreviated weekday plus time ("Sat 3:00 PM"), as in the alarm title.
pub fn weekday_time(dt: &NaiveDateTime, lang: Language) -> String {
    let i = dt.weekday().num_days_from_monday() as usize;
    let day = match lang {
        Language::En => WEEKDAYS_EN[i],
        Language::Pt => WEEKDAYS_PT[i],
        Language::De => WEEKDAYS_DE[i],
        Language::Es => WEEKDAYS_ES[i],
        Language::Zh => WEEKDAYS_ZH[i],
    };
    format!("{} {}", day, format_time(dt.hour(), dt.minute(), lang))
}

/// Numeric date and time (`.formatted(.dateTime)`), used for the JWT claims.
pub fn date_numeric(dt: &NaiveDateTime, lang: Language) -> String {
    let (d, mo, y) = (dt.day(), dt.month(), dt.year());
    let (h, mi) = (dt.hour(), dt.minute());
    match lang {
        Language::En => format!("{}/{}/{}, {}", mo, d, y, format_time(h, mi, lang)),
        Language::Pt => format!("{:02}/{:02}/{} {}", d, mo, y, format_time(h, mi, lang)),
        Language::De => format!("{:02}.{:02}.{}, {}", d, mo, y, format_time(h, mi, lang)),
        Language::Es => format!("{}/{}/{}, {}", d, mo, y, format_time(h, mi, lang)),
        Language::Zh => format!("{}/{}/{} {}", y, mo, d, format_time(h, mi, lang)),
    }
}

/// Day, month, year, hour, minute and second (the Unix timestamp item).
pub fn date_long_secs(dt: &NaiveDateTime, lang: Language) -> String {
    let (d, mo, y) = (dt.day(), dt.month() as usize, dt.year());
    let t = format_time_secs(dt.hour(), dt.minute(), dt.second(), lang);
    match lang {
        Language::En => format!("{} {}, {} at {}", MONTHS_EN[mo - 1], d, y, t),
        Language::Pt => format!("{} de {} de {} {}", d, MONTHS_PT[mo - 1], y, t),
        Language::De => format!("{:02}.{:02}.{}, {}", d, mo, y, t),
        Language::Es => format!("{} {} {}, {}", d, MONTHS_ES[mo - 1], y, t),
        Language::Zh => format!("{}年{}月{}日 {}", y, mo, d, t),
    }
}

fn currency_symbol(code: &str, lang: Language) -> String {
    let s = match code {
        "USD" => match lang {
            Language::En | Language::De => "$",
            _ => "US$",
        },
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => match lang {
            Language::En | Language::Zh => "¥",
            _ => "JP¥",
        },
        "CNY" => match lang {
            Language::Zh => "¥",
            _ => "CN¥",
        },
        "BRL" => "R$",
        "CAD" => "CA$",
        "AUD" => "A$",
        "MXN" => "MX$",
        "NZD" => "NZ$",
        "HKD" => "HK$",
        "INR" => "₹",
        "KRW" => "₩",
        other => other,
    };
    s.to_string()
}

fn group(int_part: &str, sep: char) -> String {
    let mut out = String::new();
    let n = int_part.len();
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (n - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// "$100.00", "R$ 100,00", "100,00 €": currency text for a value, like Foundation's currency style.
pub fn currency_text(value: f64, code: &str, lang: Language) -> String {
    let decimals = if matches!(code, "JPY" | "KRW" | "CLP") { 0 } else { 2 };
    let abs = format!("{:.*}", decimals, value.abs());
    let (int_part, frac) = match abs.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (abs.clone(), None),
    };
    let (group_sep, dec_sep) = match lang {
        Language::Pt | Language::De | Language::Es => ('.', ','),
        _ => (',', '.'),
    };
    let mut num = group(&int_part, group_sep);
    if let Some(f) = frac {
        num.push(dec_sep);
        num.push_str(&f);
    }
    let neg = if value < 0.0 { "-" } else { "" };
    let sym = currency_symbol(code, lang);
    let nbsp = '\u{a0}';
    let sym_is_code = sym == code;
    match lang {
        Language::En | Language::Zh => {
            if sym_is_code {
                format!("{}{}{}{}", neg, sym, nbsp, num)
            } else {
                format!("{}{}{}", neg, sym, num)
            }
        }
        Language::Pt => format!("{}{}{}{}", neg, sym, nbsp, num),
        Language::De | Language::Es => format!("{}{}{}{}", neg, num, nbsp, sym),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(format_number(90.0, 10, Language::En), "90");
        assert_eq!(format_number(90.5, 10, Language::En), "90.5");
        assert_eq!(format_number(90.5, 10, Language::Pt), "90,5");
        assert_eq!(format_number(0.1 + 0.2, 10, Language::En), "0.3");
        assert_eq!(format_number(1e16, 10, Language::En), "1e+16");
        assert_eq!(format_number(2.5e-10, 10, Language::En), "2.5e-10");
        assert_eq!(format_number(-0.0, 10, Language::En), "0");
    }

    #[test]
    fn times() {
        assert_eq!(format_time(15, 0, Language::En), "3:00 PM");
        assert_eq!(format_time(0, 5, Language::En), "12:05 AM");
        assert_eq!(format_time(15, 0, Language::Pt), "15:00");
    }

    #[test]
    fn currencies() {
        assert_eq!(currency_text(100.0, "USD", Language::En), "$100.00");
        assert_eq!(currency_text(1234.5, "BRL", Language::En), "R$1,234.50");
        assert_eq!(currency_text(1234.5, "BRL", Language::Pt), "R$\u{a0}1.234,50");
        assert_eq!(currency_text(5.0, "EUR", Language::De), "5,00\u{a0}€");
        assert_eq!(currency_text(500.0, "JPY", Language::En), "¥500");
    }
}
