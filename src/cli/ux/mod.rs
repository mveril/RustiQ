pub(crate) mod banner;
pub mod basis_table;
pub(crate) mod bat;
pub(crate) mod calculation_presentation;
pub(crate) mod calculation_report;
pub(crate) mod json_output;
pub(crate) mod mp2_report;
pub mod scf_report;
#[cfg(test)]
mod test_support;
pub(crate) use banner::print_startup_banner;
pub(crate) use basis_table::BasisTableItem;

/// Format elapsed time using Jiff's human-readable duration representation.
pub(crate) fn format_duration(duration: std::time::Duration) -> String {
    jiff::fmt::friendly::SpanPrinter::new().unsigned_duration_to_string(&duration)
}

#[cfg(test)]
mod duration_tests {
    use super::format_duration;
    use std::time::Duration;

    #[test]
    fn elapsed_durations_keep_units_and_precision() {
        assert_eq!(format_duration(Duration::ZERO), "0s");
        assert_eq!(format_duration(Duration::from_nanos(1)), "1ns");
        assert_eq!(
            format_duration(Duration::new(61, 2_003_004)),
            "1m 1s 2ms 3µs 4ns"
        );
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h");
        assert_eq!(
            format_duration(Duration::MAX),
            "5124095576030431h 15s 999ms 999µs 999ns"
        );
    }
}
