#![forbid(unsafe_code)]
//! Performance budgets, benchmark helpers, and stability gates for `Hawk2UI`.

pub mod budgets;

pub use budgets::{
    BudgetUnit, PerformanceBudget, PerformanceBudgets, PerformanceCategory, PerformanceError,
};

#[cfg(test)]
mod tests {
    use super::*;

    const BUDGETS: &str = include_str!("../../../performance/budgets.toml");

    #[test]
    fn loads_release_gate_budgets() {
        let budgets = PerformanceBudgets::parse(BUDGETS).expect("performance budgets parse");

        assert!(budgets.contains("cold-start"));
        assert!(budgets.contains("layout-pass"));
        assert!(budgets.release_gates().all(|budget| budget.release_gate));
    }

    #[test]
    fn rejects_duplicate_budget_names() {
        let duplicate = r#"
            [[budgets]]
            name = "frame-render"
            category = "rendering"
            unit = "milliseconds"
            target = 8
            maximum = 16
            release_gate = true
            fixture = "examples/style-gallery"

            [[budgets]]
            name = "frame-render"
            category = "rendering"
            unit = "milliseconds"
            target = 8
            maximum = 16
            release_gate = true
            fixture = "examples/style-gallery"
        "#;

        let error = PerformanceBudgets::parse(duplicate).expect_err("duplicate budget must fail");
        assert_eq!(
            error,
            PerformanceError::DuplicateBudget("frame-render".to_owned())
        );
    }

    #[test]
    fn rejects_target_above_maximum() {
        let invalid = r#"
            [[budgets]]
            name = "layout-pass"
            category = "layout"
            unit = "milliseconds"
            target = 12
            maximum = 8
            release_gate = true
            fixture = "examples/desktop-dashboard"
        "#;

        let error = PerformanceBudgets::parse(invalid).expect_err("target above max must fail");
        assert_eq!(
            error,
            PerformanceError::TargetExceedsMaximum("layout-pass".to_owned())
        );
    }
}
