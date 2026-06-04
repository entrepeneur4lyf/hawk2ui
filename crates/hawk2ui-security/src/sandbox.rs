//! Script sandbox diagnostic evidence.
//!
//! The script runtime enforces host-call and execution policy. This module records sandbox denial
//! decisions in a stable diagnostic vocabulary after an enforcing layer has made the decision.

use crate::{SecurityDiagnostic, SecuritySeverity};

/// Direct script operation denied by the sandbox.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptSandboxOperation {
    /// String-to-code execution such as eval.
    StringToCode,
    /// Host API access without declared capability.
    UndeclaredHostApi,
    /// Direct filesystem access.
    DirectFilesystem,
    /// Direct network access.
    DirectNetwork,
    /// Process spawning.
    ProcessSpawning,
    /// Native module loading.
    NativeModuleLoading,
}

impl ScriptSandboxOperation {
    fn diagnostic(self) -> SecurityDiagnostic {
        let (rule, message) = match self {
            Self::StringToCode => (
                "script.string-to-code.denied",
                "string-to-code execution is denied",
            ),
            Self::UndeclaredHostApi => (
                "script.host-api.undeclared",
                "host API access is undeclared",
            ),
            Self::DirectFilesystem => (
                "script.filesystem.denied",
                "direct filesystem access is denied",
            ),
            Self::DirectNetwork => ("script.network.denied", "direct network access is denied"),
            Self::ProcessSpawning => ("script.process.denied", "process spawning is denied"),
            Self::NativeModuleLoading => (
                "script.native-module.denied",
                "native module loading is denied",
            ),
        };
        SecurityDiagnostic::new(SecuritySeverity::Error, rule, message)
    }
}

/// Script sandbox denial record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptSandboxDenial {
    /// Denied operation.
    pub operation: ScriptSandboxOperation,
    /// Script or module that attempted the operation.
    pub subject: String,
    /// Structured diagnostic.
    pub diagnostic: SecurityDiagnostic,
}

impl ScriptSandboxDenial {
    /// Returns a stable diagnostic label for the sandbox denial.
    #[must_use]
    pub fn diagnostic_label(&self) -> String {
        format!("sandbox.{}:{}", self.diagnostic.rule, self.subject)
    }
}

/// Script sandbox denial evidence factory.
pub struct ScriptSandboxPolicy;

impl ScriptSandboxPolicy {
    /// Produces a denial evidence record for a direct privileged script operation.
    #[must_use]
    pub fn deny(
        operation: ScriptSandboxOperation,
        subject: impl Into<String>,
    ) -> ScriptSandboxDenial {
        ScriptSandboxDenial {
            operation,
            subject: subject.into(),
            diagnostic: operation.diagnostic(),
        }
    }
}
