use serde::{Deserialize, Serialize};
use unio_protocol::PermissionMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    ReadWorkspace,
    WriteWorkspace,
    ExecuteProcess,
    NetworkAccess,
    PlanOnly,
    SkillTool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolPrecheck {
    pub capability: ToolCapability,
    pub risk: RiskLevel,
    pub inside_workspace: bool,
    pub trusted_network: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityDecision {
    Allow,
    RequireApproval { reason: String },
    Deny { reason: String },
}

pub fn decide(mode: PermissionMode, check: &ToolPrecheck) -> SecurityDecision {
    match mode {
        PermissionMode::FullTrust => SecurityDecision::Allow,
        PermissionMode::Default => default_decision(check),
        PermissionMode::Auto => auto_decision(check),
    }
}

fn default_decision(check: &ToolPrecheck) -> SecurityDecision {
    match check.capability {
        ToolCapability::ReadWorkspace | ToolCapability::PlanOnly if check.inside_workspace => {
            SecurityDecision::Allow
        }
        ToolCapability::ReadWorkspace => SecurityDecision::Deny {
            reason: "default mode cannot read outside workspace".into(),
        },
        ToolCapability::WriteWorkspace
        | ToolCapability::ExecuteProcess
        | ToolCapability::NetworkAccess
        | ToolCapability::SkillTool => SecurityDecision::RequireApproval {
            reason: "default mode requires approval for side effects".into(),
        },
        ToolCapability::PlanOnly => SecurityDecision::Allow,
    }
}

fn auto_decision(check: &ToolPrecheck) -> SecurityDecision {
    match check.capability {
        ToolCapability::ReadWorkspace | ToolCapability::PlanOnly => SecurityDecision::Allow,
        ToolCapability::WriteWorkspace
            if check.inside_workspace && check.risk == RiskLevel::Low =>
        {
            SecurityDecision::Allow
        }
        ToolCapability::NetworkAccess if check.trusted_network && check.risk == RiskLevel::Low => {
            SecurityDecision::Allow
        }
        _ => SecurityDecision::RequireApproval {
            reason: "auto mode requires approval for elevated risk".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use unio_protocol::PermissionMode;

    use super::{decide, RiskLevel, SecurityDecision, ToolCapability, ToolPrecheck};

    #[test]
    fn permission_policy_matrix_matches_v1_rules() {
        let write = ToolPrecheck {
            capability: ToolCapability::WriteWorkspace,
            risk: RiskLevel::Low,
            inside_workspace: true,
            trusted_network: false,
        };

        assert!(matches!(
            decide(PermissionMode::Default, &write),
            SecurityDecision::RequireApproval { .. }
        ));
        assert_eq!(
            decide(PermissionMode::Auto, &write),
            SecurityDecision::Allow
        );
        assert_eq!(
            decide(PermissionMode::FullTrust, &write),
            SecurityDecision::Allow
        );
    }
}
