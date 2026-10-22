use crate::models::submission::{Submission, SubmissionEvent, Submitter};
use crate::pdf::generator::build_simple_pdf;

/// Generates a dedicated Audit Trail Certificate PDF containing submission metadata,
/// SHA-256 document hash, participant details (Name, Email, Role, IP, User-Agent, Signature ID),
/// and a chronological timeline of all `SubmissionEvent` records.
pub fn generate_audit_trail_pdf(
    submission: &Submission,
    submitters: &[Submitter],
    events: &[SubmissionEvent],
    document_sha256: &str,
) -> Vec<u8> {
    let mut lines = Vec::new();
    lines.push(format!("Submission Name: {}", submission.name));
    lines.push(format!("Submission ID: {} (Slug: {})", submission.id, submission.slug));
    lines.push(format!("Document SHA-256: {}", document_sha256));
    lines.push(format!(
        "Completed Timestamp: {}",
        if submission.completed_at.is_empty() {
            chrono::Utc::now().to_rfc3339()
        } else {
            submission.completed_at.clone()
        }
    ));
    lines.push(" ".to_string());
    lines.push("=== SIGNER PARTICIPANTS & VERIFICATION ===".to_string());

    for (idx, sub) in submitters.iter().enumerate() {
        lines.push(format!(
            "{}. {} <{}> | Role: {} | Status: {}",
            idx + 1,
            if sub.name.is_empty() { "Recipient" } else { &sub.name },
            sub.email,
            sub.role(),
            sub.status()
        ));
        lines.push(format!(
            "   Signature ID: {} | IP: {} | Timezone: {}",
            sub.uuid,
            if sub.ip.is_empty() { "127.0.0.1" } else { &sub.ip },
            if sub.timezone.is_empty() { "UTC" } else { &sub.timezone }
        ));
        if !sub.ua.is_empty() {
            let ua_short: String = sub.ua.chars().take(72).collect();
            lines.push(format!("   User-Agent: {}", ua_short));
        }
        if !sub.completed_at.is_empty() {
            lines.push(format!("   Completed At: {}", sub.completed_at));
        }
    }

    lines.push(" ".to_string());
    lines.push("=== CHRONOLOGICAL AUDIT EVENT LOG ===".to_string());

    for ev in events {
        let ts = if ev.event_timestamp.is_empty() {
            &ev.created
        } else {
            &ev.event_timestamp
        };
        lines.push(format!(
            "[{}] Event: {} (Submitter: {})",
            ts,
            ev.event_type,
            if ev.submitter_id.is_empty() {
                "system"
            } else {
                &ev.submitter_id
            }
        ));
    }

    build_simple_pdf("AUDIT TRAIL CERTIFICATE", &lines)
}
