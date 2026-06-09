#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameworkPackageVersions {
    version: &'static str,
}

impl FrameworkPackageVersions {
    pub(crate) const fn from_cli_version() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    pub(crate) fn dependency_range(self) -> String {
        format!("^{}", self.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_range_uses_cli_package_version() {
        assert_eq!(
            FrameworkPackageVersions::from_cli_version().dependency_range(),
            format!("^{}", env!("CARGO_PKG_VERSION"))
        );
    }
}
