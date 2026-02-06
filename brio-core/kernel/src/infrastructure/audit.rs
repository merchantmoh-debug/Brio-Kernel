use serde::Serialize;
use tracing::{info, info_span};

/// Domain event for audit logging.
/// Structured for JSON serialization to enable machine-readable audit trails.
#[derive(Debug, Serialize)]
#[serde(tag = "event_type", rename_all = "snake_case")]
pub enum AuditEvent {
    SystemStartup {
        component: String,
    },
    SystemShutdown {
        reason: String,
    },
    AccessDenied {
        user: String,
        resource: String,
    },
    ConfigChanged {
        key: String,
        old_val: String,
        new_val: String,
    },
}

/// Logs an audit event to the dedicated audit channel as structured JSON.
/// This uses a specific `target` which can be filtered by the subscriber to redirect to a secure file.
pub fn log_audit(event: &AuditEvent) {
    let span = info_span!(target: "audit", "audit_event");
    let _enter = span.enter();

    // Serialize to JSON for machine-readable audit logs
    let json = serde_json::to_string(event).unwrap_or_else(|e| format!("{{\"error\": \"{e}\"}}"));
    info!(target: "audit", audit_json = %json, "Security Audit Event");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_audit_variants() {
        // These calls should not panic
        log_audit(&AuditEvent::SystemStartup {
            component: "Test".into(),
        });
        log_audit(&AuditEvent::SystemShutdown {
            reason: "Testing".into(),
        });
        log_audit(&AuditEvent::AccessDenied {
            user: "bob".into(),
            resource: "secret".into(),
        });
        log_audit(&AuditEvent::ConfigChanged {
            key: "port".into(),
            old_val: "80".into(),
            new_val: "8080".into(),
        });
    }
}
