use crate::query::QueryClassification;

/// The safety verdict for a query given the current write policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyLevel {
    /// Query is safe to execute without confirmation.
    Safe,
    /// Query requires explicit user confirmation before execution.
    RequiresConfirmation,
    /// Query is blocked and cannot execute under current policy.
    Blocked,
}

/// The active write policy for the current session/connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePolicy {
    /// No writes allowed. Production default.
    ReadOnly,
    /// Writes require confirmation dialog.
    ConfirmWrites,
    /// All queries allowed without extra friction.
    AllowAll,
}

/// Evaluate the safety level for a query given its classification and the active write policy.
///
/// This is advisory — not a security boundary. The execution layer should also
/// use database-level protections (e.g., read-only transactions) where possible.
pub fn evaluate_safety(classification: QueryClassification, policy: WritePolicy) -> SafetyLevel {
    match policy {
        WritePolicy::AllowAll => SafetyLevel::Safe,

        WritePolicy::ReadOnly => match classification {
            QueryClassification::Read | QueryClassification::TransactionControl => {
                SafetyLevel::Safe
            }
            _ => SafetyLevel::Blocked,
        },

        WritePolicy::ConfirmWrites => match classification {
            QueryClassification::Read | QueryClassification::TransactionControl => {
                SafetyLevel::Safe
            }
            QueryClassification::Write | QueryClassification::Unknown => {
                SafetyLevel::RequiresConfirmation
            }
            QueryClassification::Destructive => SafetyLevel::RequiresConfirmation,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_blocks_writes() {
        assert_eq!(
            evaluate_safety(QueryClassification::Write, WritePolicy::ReadOnly),
            SafetyLevel::Blocked
        );
        assert_eq!(
            evaluate_safety(QueryClassification::Destructive, WritePolicy::ReadOnly),
            SafetyLevel::Blocked
        );
        assert_eq!(
            evaluate_safety(QueryClassification::Unknown, WritePolicy::ReadOnly),
            SafetyLevel::Blocked
        );
    }

    #[test]
    fn read_only_allows_reads() {
        assert_eq!(
            evaluate_safety(QueryClassification::Read, WritePolicy::ReadOnly),
            SafetyLevel::Safe
        );
        assert_eq!(
            evaluate_safety(
                QueryClassification::TransactionControl,
                WritePolicy::ReadOnly
            ),
            SafetyLevel::Safe
        );
    }

    #[test]
    fn confirm_writes_requires_confirmation() {
        assert_eq!(
            evaluate_safety(QueryClassification::Write, WritePolicy::ConfirmWrites),
            SafetyLevel::RequiresConfirmation
        );
        assert_eq!(
            evaluate_safety(QueryClassification::Destructive, WritePolicy::ConfirmWrites),
            SafetyLevel::RequiresConfirmation
        );
    }

    #[test]
    fn confirm_writes_allows_reads() {
        assert_eq!(
            evaluate_safety(QueryClassification::Read, WritePolicy::ConfirmWrites),
            SafetyLevel::Safe
        );
    }

    #[test]
    fn allow_all_permits_everything() {
        assert_eq!(
            evaluate_safety(QueryClassification::Destructive, WritePolicy::AllowAll),
            SafetyLevel::Safe
        );
        assert_eq!(
            evaluate_safety(QueryClassification::Write, WritePolicy::AllowAll),
            SafetyLevel::Safe
        );
        assert_eq!(
            evaluate_safety(QueryClassification::Read, WritePolicy::AllowAll),
            SafetyLevel::Safe
        );
    }
}
