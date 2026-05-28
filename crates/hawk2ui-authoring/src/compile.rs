//! Source-to-authoring-record compilation entrypoint.

use hawk2ui_api::Diagnostic;

use crate::{
    ChildList, ComponentId, ComponentInstance, ElementId, ElementKind, ElementNode, EventBinding,
    EventKind, HandlerRef, PropValue,
};

/// Authoring diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthoringDiagnosticSeverity {
    /// Release-blocking authoring error.
    Error,
    /// Non-blocking authoring warning.
    Warning,
}

/// Structured authoring diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthoringDiagnostic {
    /// Diagnostic severity.
    pub severity: AuthoringDiagnosticSeverity,
    /// Stable diagnostic rule.
    pub rule: String,
    /// Human-readable diagnostic message.
    pub message: String,
}

impl AuthoringDiagnostic {
    /// Creates an authoring diagnostic.
    #[must_use]
    pub fn new(
        severity: AuthoringDiagnosticSeverity,
        rule: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            rule: rule.into(),
            message: message.into(),
        }
    }
}

impl From<AuthoringDiagnostic> for Diagnostic {
    fn from(diagnostic: AuthoringDiagnostic) -> Self {
        match diagnostic.severity {
            AuthoringDiagnosticSeverity::Error => Self::error(diagnostic.rule, diagnostic.message),
            AuthoringDiagnosticSeverity::Warning => {
                Self::warning(diagnostic.rule, diagnostic.message)
            }
        }
    }
}

/// Compiled authoring artifact.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthoringArtifact {
    components: Vec<ComponentInstance>,
    events: Vec<EventBinding>,
    diagnostics: Vec<AuthoringDiagnostic>,
}

impl AuthoringArtifact {
    /// Creates an authoring artifact.
    #[must_use]
    pub fn new(
        components: Vec<ComponentInstance>,
        events: Vec<EventBinding>,
        diagnostics: Vec<AuthoringDiagnostic>,
    ) -> Self {
        Self {
            components,
            events,
            diagnostics,
        }
    }

    /// Returns compiled component instances.
    #[must_use]
    pub fn components(&self) -> &[ComponentInstance] {
        &self.components
    }

    /// Returns compiled event bindings.
    #[must_use]
    pub fn events(&self) -> &[EventBinding] {
        &self.events
    }

    /// Returns compiler diagnostics.
    #[must_use]
    pub fn diagnostics(&self) -> &[AuthoringDiagnostic] {
        &self.diagnostics
    }
}

/// Compiles Hawk authoring source into typed records.
#[must_use]
pub fn compile_authoring_source(
    input: &str,
    diagnostics: &mut Vec<AuthoringDiagnostic>,
) -> AuthoringArtifact {
    let mut compiler = Compiler::default();
    compiler.compile(input);
    diagnostics.extend(compiler.diagnostics.clone());
    AuthoringArtifact::new(compiler.components, compiler.events, compiler.diagnostics)
}

#[derive(Default)]
struct Compiler {
    current_component: Option<CurrentComponent>,
    components: Vec<ComponentInstance>,
    events: Vec<EventBinding>,
    diagnostics: Vec<AuthoringDiagnostic>,
}

impl Compiler {
    fn compile(&mut self, input: &str) {
        for (line_index, raw_line) in input.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line == "{" {
                continue;
            }
            if line == "}" {
                self.finish_component();
                continue;
            }
            if let Some(rest) = line.strip_prefix("component ") {
                self.start_component(rest, line_index + 1);
            } else if let Some(rest) = line.strip_prefix("text ") {
                self.add_text(rest, line_index + 1);
            } else if let Some(rest) = line.strip_prefix("on ") {
                self.add_event(rest, line_index + 1);
            } else {
                self.error(
                    "authoring.syntax.unknown-line",
                    format!("unknown authoring syntax on line {}", line_index + 1),
                );
            }
        }
        self.finish_component();
    }

    fn start_component(&mut self, rest: &str, line: usize) {
        self.finish_component();
        let mut parts = rest.split_whitespace();
        let Some(component_name) = parts.next() else {
            self.error(
                "authoring.component.name.missing",
                format!("component name is missing on line {line}"),
            );
            return;
        };
        let id = parts
            .find_map(|part| part.strip_prefix("id="))
            .map_or_else(|| component_name.to_string(), ToString::to_string);
        self.current_component = Some(CurrentComponent {
            id: ComponentId::new(id),
            component_name: component_name.to_string(),
            default_children: Vec::new(),
        });
    }

    fn add_text(&mut self, rest: &str, line: usize) {
        let Some(component) = self.current_component.as_mut() else {
            self.error(
                "authoring.text.outside-component",
                format!("text node declared outside component on line {line}"),
            );
            return;
        };
        let Some((id, quoted)) = rest.split_once(' ') else {
            self.error(
                "authoring.text.invalid",
                format!("text node is missing content on line {line}"),
            );
            return;
        };
        let text = quoted.trim().trim_matches('"');
        component.default_children.push(
            ElementNode::new(ElementId::new(id), ElementKind::Text)
                .with_prop("text", PropValue::String(text.to_string())),
        );
    }

    fn add_event(&mut self, rest: &str, line: usize) {
        let Some(component) = self.current_component.as_ref() else {
            self.error(
                "authoring.event.outside-component",
                format!("event binding declared outside component on line {line}"),
            );
            return;
        };
        let mut parts = rest.split_whitespace();
        let Some(event_name) = parts.next() else {
            self.error(
                "authoring.event.name.missing",
                format!("event name is missing on line {line}"),
            );
            return;
        };
        let Some(handler) = parts.next() else {
            self.error(
                "authoring.event.handler.missing",
                format!("event handler is missing on line {line}"),
            );
            return;
        };
        let Some(event) = parse_event(event_name) else {
            self.error(
                "authoring.event.unsupported",
                format!("unsupported event `{event_name}` on line {line}"),
            );
            return;
        };
        self.events.push(EventBinding::new(
            ElementId::new(component.id.as_str()),
            event,
            HandlerRef::new(handler),
        ));
    }

    fn finish_component(&mut self) {
        if let Some(component) = self.current_component.take() {
            match ChildList::ordered(component.default_children) {
                Ok(children) => {
                    self.components.push(
                        ComponentInstance::new(component.id, component.component_name)
                            .with_slot("default", children),
                    );
                }
                Err(error) => {
                    self.error(
                        "authoring.children.duplicate-key",
                        format!("duplicate child key `{}`", error.duplicate_key()),
                    );
                }
            }
        }
    }

    fn error(&mut self, rule: impl Into<String>, message: impl Into<String>) {
        self.diagnostics.push(AuthoringDiagnostic::new(
            AuthoringDiagnosticSeverity::Error,
            rule,
            message,
        ));
    }
}

struct CurrentComponent {
    id: ComponentId,
    component_name: String,
    default_children: Vec<ElementNode>,
}

fn parse_event(event_name: &str) -> Option<EventKind> {
    event_name.parse().ok()
}
