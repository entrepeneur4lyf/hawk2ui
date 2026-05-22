#![forbid(unsafe_code)]
//! Workspace maintenance and release automation commands for Hawk2UI.

const CRATE_NAME: &str = "xtask";

fn main() {
    println!("{CRATE_NAME}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_binary_identity() {
        assert_eq!(CRATE_NAME, "xtask");
    }
}
