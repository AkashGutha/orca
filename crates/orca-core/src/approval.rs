use crate::agent::ApprovalMode;

#[derive(Debug, Clone, Copy)]
pub struct ApprovalGate {
    mode: ApprovalMode,
    approved_by_cli: bool,
}

impl ApprovalGate {
    pub fn new(mode: ApprovalMode, approved_by_cli: bool) -> Self {
        Self {
            mode,
            approved_by_cli,
        }
    }

    pub fn approved(self) -> bool {
        match self.mode {
            ApprovalMode::Auto => true,
            ApprovalMode::Manual => self.approved_by_cli,
        }
    }
}
