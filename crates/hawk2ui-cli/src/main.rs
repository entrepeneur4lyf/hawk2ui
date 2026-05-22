#![forbid(unsafe_code)]
//! Command-line interface for Hawk2UI validation, builds, runs, packaging, and diagnostics.

const CRATE_NAME: &str = "hawk2ui-cli";

fn main() {
    println!("{CRATE_NAME}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binary_identity() {
        assert_eq!(CRATE_NAME, "hawk2ui-cli");
    }
}
