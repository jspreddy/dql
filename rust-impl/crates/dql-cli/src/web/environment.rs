//! Header environment for the web UI.
//!
//! Local DynamoDB shows `host:port`. A live session shows the aws-vault
//! profile and region, and keeps the access key masked. Secrets and session
//! tokens are never read.

use dql_engine::CallerIdentity;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvFacts {
    pub vault: Option<String>,
    pub region: Option<String>,
    pub default_region: Option<String>,
    pub access_key_id: Option<String>,
    pub expiration: Option<String>,
}

impl EnvFacts {
    pub fn from_env() -> Self {
        Self {
            vault: nonempty("AWS_VAULT"),
            region: nonempty("AWS_REGION"),
            default_region: nonempty("AWS_DEFAULT_REGION"),
            access_key_id: nonempty("AWS_ACCESS_KEY_ID"),
            expiration: nonempty("AWS_CREDENTIAL_EXPIRATION"),
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct EnvironmentView {
    pub kind: &'static str,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<String>,
}

pub fn environment_view(
    local: Option<(&str, u16)>,
    session_region: &str,
    memory: bool,
    facts: &EnvFacts,
) -> EnvironmentView {
    if let Some((host, port)) = local {
        return EnvironmentView {
            kind: "local",
            label: format!("{host}:{port}"),
            profile: None,
            region: None,
            access_key: None,
            expiration: None,
        };
    }
    if memory {
        return EnvironmentView {
            kind: "memory",
            label: "memory".to_string(),
            profile: None,
            region: None,
            access_key: None,
            expiration: None,
        };
    }
    let region = facts
        .region
        .clone()
        .or_else(|| facts.default_region.clone())
        .or_else(|| {
            let region = session_region.trim();
            if region.is_empty() {
                None
            } else {
                Some(region.to_string())
            }
        });
    let profile = facts.vault.clone();
    let label = match (&profile, &region) {
        (Some(profile), Some(region)) => format!("{profile} · {region}"),
        (Some(profile), None) => profile.clone(),
        (None, Some(region)) => region.clone(),
        (None, None) => "aws".to_string(),
    };
    EnvironmentView {
        kind: "aws",
        label,
        profile,
        region,
        access_key: facts.access_key_id.as_deref().map(mask_access_key),
        expiration: facts.expiration.clone(),
    }
}

pub fn identity_payload(identity: &CallerIdentity) -> Value {
    json!({
        "ok": true,
        "account": identity.account,
        "arn": identity.arn,
        "user_id": identity.user_id,
        "principal": principal_from_arn(&identity.arn),
    })
}

pub fn principal_from_arn(arn: &str) -> String {
    let resource = arn.split(':').nth(5).unwrap_or("").trim();
    if resource.is_empty() {
        return String::new();
    }
    let Some((kind, name)) = resource.split_once('/') else {
        return resource.to_string();
    };
    if kind == "assumed-role" {
        let mut parts = name.split('/');
        let role = parts.next().unwrap_or(name);
        return match parts.next() {
            Some(session) if !session.is_empty() => format!("{role} ({session})"),
            _ => role.to_string(),
        };
    }
    name.rsplit('/').next().unwrap_or(name).to_string()
}

fn mask_access_key(key: &str) -> String {
    let key = key.trim();
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 8 {
        return "••••".to_string();
    }
    let prefix: String = chars.iter().take(4).collect();
    let suffix: String = chars.iter().skip(chars.len() - 4).collect();
    format!("{prefix}…{suffix}")
}

fn nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> EnvFacts {
        EnvFacts {
            vault: Some("jonsmith".to_string()),
            region: Some("us-east-1".to_string()),
            default_region: Some("us-west-2".to_string()),
            access_key_id: Some("AKIAIOSFODNN7EXAMPLE".to_string()),
            expiration: Some("2020-04-16T11:16:27Z".to_string()),
        }
    }

    #[test]
    fn local_endpoint_is_only_host_and_port() {
        let view = environment_view(Some(("localhost", 8000)), "us-east-1", false, &facts());
        assert_eq!(view.kind, "local");
        assert_eq!(view.label, "localhost:8000");
        assert!(view.profile.is_none());
        assert!(view.access_key.is_none());
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("AKIA"));
        assert!(!json.contains("jonsmith"));
    }

    #[test]
    fn memory_hides_ambient_vault_variables() {
        let view = environment_view(None, "us-east-1", true, &facts());
        assert_eq!(view.kind, "memory");
        assert_eq!(view.label, "memory");
        assert!(view.profile.is_none());
    }

    #[test]
    fn aws_label_uses_vault_profile_and_region() {
        let view = environment_view(None, "eu-west-1", false, &facts());
        assert_eq!(view.kind, "aws");
        assert_eq!(view.label, "jonsmith · us-east-1");
        assert_eq!(view.profile.as_deref(), Some("jonsmith"));
        assert_eq!(view.region.as_deref(), Some("us-east-1"));
        assert_eq!(view.access_key.as_deref(), Some("AKIA…MPLE"));
        assert_eq!(view.expiration.as_deref(), Some("2020-04-16T11:16:27Z"));
        let json = serde_json::to_string(&view).unwrap();
        assert!(!json.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn aws_falls_back_to_default_region_then_the_session() {
        let mut bare = EnvFacts {
            vault: None,
            region: None,
            default_region: Some("us-west-2".to_string()),
            access_key_id: Some("short".to_string()),
            expiration: None,
        };
        let view = environment_view(None, "eu-west-1", false, &bare);
        assert_eq!(view.label, "us-west-2");
        assert_eq!(view.access_key.as_deref(), Some("••••"));

        bare.default_region = None;
        let view = environment_view(None, "eu-west-1", false, &bare);
        assert_eq!(view.label, "eu-west-1");
        assert!(view.profile.is_none());
        assert!(view.expiration.is_none());
    }

    #[test]
    fn principal_comes_from_the_caller_arn() {
        assert_eq!(
            principal_from_arn("arn:aws:iam::123456789012:user/jonsmith"),
            "jonsmith"
        );
        assert_eq!(
            principal_from_arn("arn:aws:iam::123456789012:user/team/jonsmith"),
            "jonsmith"
        );
        assert_eq!(
            principal_from_arn("arn:aws:sts::123456789012:assumed-role/Admin/jonsmith"),
            "Admin (jonsmith)"
        );
        assert_eq!(
            principal_from_arn("arn:aws:iam::123456789012:role/Admin"),
            "Admin"
        );
        assert_eq!(principal_from_arn("arn:aws:iam::123456789012:root"), "root");
    }
}
