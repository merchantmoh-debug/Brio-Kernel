use tracing::{info, info_span};

/// Domain event for audit logging.
/// In a real system, this would be an Enum of all possible security events.
#[derive(Debug)]
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

/// Logs an audit event to the dedicated audit channel.
/// This uses a specific `target` which can be filtered by the subscriber to redirect to a secure file.
pub fn log_audit(event: AuditEvent) {
    let span = info_span!(target: "audit", "audit_event");
    let _enter = span.enter();

    // We log the event as a structured field.
    // In production, we'd implementation `Serialize` for AuditEvent and log it as json.
    info!(target: "audit", event = ?event, "Security Audit Event");
}
