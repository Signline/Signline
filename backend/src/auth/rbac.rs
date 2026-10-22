use crate::error::{AppError, AppResult};
use crate::models::user::User;

/// Role-based access control matching Signline's 4 roles:
/// - `admin`: Full account, settings, team, billing, templates, and submissions control.
/// - `manager`: Full templates and submissions management across the account, plus profile settings.
/// - `member`: Create/manage own templates and submissions, view shared templates.
/// - `agent`: Send submissions from existing templates and view own submissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Manager,
    Member,
    Agent,
}

impl Role {
    pub fn from_str(role: &str) -> Self {
        match role.trim().to_ascii_lowercase().as_str() {
            "admin" => Role::Admin,
            "manager" => Role::Manager,
            "member" => Role::Member,
            "agent" => Role::Agent,
            _ => Role::Admin,
        }
    }

    pub fn can_manage_account(&self) -> bool {
        matches!(self, Role::Admin)
    }

    pub fn can_manage_users(&self) -> bool {
        matches!(self, Role::Admin)
    }

    pub fn can_manage_all_templates(&self) -> bool {
        matches!(self, Role::Admin | Role::Manager)
    }

    pub fn can_create_templates(&self) -> bool {
        matches!(self, Role::Admin | Role::Manager | Role::Member)
    }

    pub fn can_create_submissions(&self) -> bool {
        true
    }
}

pub fn require_admin(user: &User) -> AppResult<()> {
    let role = Role::from_str(&user.role);
    if role.can_manage_account() {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "Admin privileges are required to perform this action",
        ))
    }
}

pub fn require_template_write(user: &User) -> AppResult<()> {
    let role = Role::from_str(&user.role);
    if role.can_create_templates() {
        Ok(())
    } else {
        Err(AppError::forbidden(
            "Agent role cannot create or modify templates",
        ))
    }
}
