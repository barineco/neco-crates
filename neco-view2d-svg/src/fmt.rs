use alloc::{format, string::String};

pub(crate) fn format_f64(value: f64) -> String {
    if value == 0.0 {
        return String::from("0");
    }

    let mut out = format!("{value:.6}");
    if out.contains('.') {
        while out.ends_with('0') {
            out.pop();
        }
        if out.ends_with('.') {
            out.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::format_f64;

    #[test]
    fn trims_and_rounds_to_six_digits() {
        assert_eq!(format_f64(0.0), "0");
        assert_eq!(format_f64(1.0), "1");
        assert_eq!(format_f64(-2.5), "-2.5");
        assert_eq!(format_f64(1.23456789), "1.234568");
        assert_eq!(format_f64(0.5000001), "0.5");
        assert_eq!(format_f64(100.0), "100");
        assert_eq!(format_f64(-0.0), "0");
    }
}
