//! The controlled credential services of a profile row: container, instance metadata, SSO and
//! role-token endpoints on one loopback server, and the ordered log of what reached them.

use std::sync::{Arc, Mutex};

use serde::Deserialize;

use crate::TestResult;
use crate::bedrock::credential_scope;
use crate::loopback::{Received, Reply, Routed, serve_routed};
use crate::profile_credentials::Input;

/// One role-token request as the service saw it.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RoleRequest {
    /// Requested operation.
    pub action: String,
    /// Role requested.
    pub role_arn: Option<String>,
    /// Session name requested.
    pub session: Option<String>,
    /// Requested duration in seconds, as text.
    pub duration: Option<String>,
    /// External identifier requested.
    pub external_id: Option<String>,
    /// Access key the request was signed with.
    pub source_key: Option<String>,
    /// Region the request was signed for.
    pub region: Option<String>,
}

/// One SSO role-credentials request as the service saw it.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SsoRequest {
    /// Account requested.
    pub account: Option<String>,
    /// Role requested.
    pub role: Option<String>,
}

/// The Bedrock request the conversation finally sent.
#[derive(Debug, PartialEq)]
pub struct Sent {
    /// First part of the Authorization header.
    pub authorization: Option<String>,
    /// Security token header.
    pub security_token: Option<String>,
}

/// Everything the services saw, in arrival order.
#[derive(Default)]
pub struct Log {
    /// Service names in arrival order, with the Bedrock request last when it was sent.
    pub sequence: Vec<String>,
    /// Role-token requests in arrival order.
    pub roles: Vec<RoleRequest>,
    /// SSO role-credentials requests in arrival order.
    pub sso: Vec<SsoRequest>,
    /// The Bedrock request, when one was sent.
    pub sent: Option<Sent>,
}

/// A JSON reply.
fn json(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "application/json",
        headers: &[],
        body: body.as_bytes().to_vec(),
    }
}

/// A plain reply.
fn text(status: u16, body: &str) -> Reply {
    Reply {
        status,
        content_type: "text/plain",
        headers: &[],
        body: body.as_bytes().to_vec(),
    }
}

/// Credentials in the JSON shape the container and instance services answer with.
fn credentials(key: &str) -> String {
    format!(
        r#"{{"AccessKeyId":"{key}","SecretAccessKey":"controlled-secret","Token":"controlled-token","Expiration":"2099-01-01T00:00:00Z"}}"#
    )
}

/// The first part of an Authorization header with its date replaced, as the rows record it.
fn undated(authorization: &str) -> String {
    let head = authorization.split(',').next().unwrap_or_default();
    head.split('/')
        .enumerate()
        .map(|(position, part)| if position == 1 { "<date>" } else { part })
        .collect::<Vec<_>>()
        .join("/")
}

/// A role-token request: record it and answer with role credentials or a denial.
fn role_token(input: &Input, request: &Received, log: &mut Log) -> Reply {
    let params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(&request.body)
            .into_owned()
            .collect();
    let action = params.get("Action").cloned().unwrap_or_default();
    log.sequence.push(action.clone());
    let scope = request.header("authorization").and_then(credential_scope);
    log.roles.push(RoleRequest {
        action: action.clone(),
        role_arn: params.get("RoleArn").cloned(),
        session: params.get("RoleSessionName").cloned(),
        duration: params.get("DurationSeconds").cloned(),
        external_id: params.get("ExternalId").cloned(),
        source_key: scope.map(|(key, _)| key.to_owned()),
        region: scope.map(|(_, region)| region.to_owned()),
    });
    let xml = |status: u16, body: String| Reply {
        status,
        content_type: "text/xml",
        headers: &[],
        body: body.into_bytes(),
    };
    if input.deny.is_some() {
        return xml(
            403,
            "<ErrorResponse><Error><Type>Sender</Type><Code>AccessDenied</Code><Message>controlled STS denial</Message></Error><RequestId>controlled</RequestId></ErrorResponse>".to_owned(),
        );
    }
    let key = match params.get("RoleArn") {
        _ if action == "AssumeRoleWithWebIdentity" => "web-key",
        Some(arn) if arn.ends_with("/base") => "base-role-key",
        _ => "role-key",
    };
    xml(
        200,
        format!(
            r#"<{action}Response xmlns="https://sts.amazonaws.com/doc/2011-06-15/"><{action}Result><Credentials><AccessKeyId>{key}</AccessKeyId><SecretAccessKey>controlled-secret</SecretAccessKey><SessionToken>role-token</SessionToken><Expiration>2099-01-01T00:00:00Z</Expiration></Credentials><AssumedRoleUser><AssumedRoleId>id:session</AssumedRoleId><Arn>arn:aws:sts::123456789012:assumed-role/role/session</Arn></AssumedRoleUser></{action}Result><ResponseMetadata><RequestId>controlled</RequestId></ResponseMetadata></{action}Response>"#
        ),
    )
}

/// An SSO role-credentials request: record it and answer with credentials, a denial, or the
/// rejection of an expired token.
fn sso_credentials(input: &Input, request: &Received, query: &str, log: &mut Log) -> Reply {
    log.sequence.push("sso".to_owned());
    let params: std::collections::HashMap<String, String> =
        url::form_urlencoded::parse(query.as_bytes())
            .into_owned()
            .collect();
    log.sso.push(SsoRequest {
        account: params.get("account_id").cloned(),
        role: params.get("role_name").cloned(),
    });
    if input.deny.is_some() {
        json(
            403,
            r#"{"__type":"UnauthorizedException","message":"controlled SSO denial"}"#,
        )
    } else if request.header("x-amz-sso_bearer_token") == Some("expired-sso-token") {
        json(
            401,
            r#"{"__type":"UnauthorizedException","message":"controlled SSO expiry"}"#,
        )
    } else {
        json(
            200,
            r#"{"roleCredentials":{"accessKeyId":"sso-key","secretAccessKey":"controlled-secret","sessionToken":"sso-token","expiration":4070908800000}}"#,
        )
    }
}

/// Answer one request and record it.
fn answer(input: &Input, request: &Received, log: &mut Log) -> Reply {
    let target = request.request_line.split(' ').nth(1).unwrap_or_default();
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    match path {
        "/container" => {
            log.sequence.push("container".to_owned());
            if input.container_fail.is_some() {
                text(400, "controlled denial")
            } else {
                json(200, &credentials("container-key"))
            }
        }
        "/latest/api/token" => {
            log.sequence.push("instance-token".to_owned());
            Reply {
                headers: &[("x-aws-ec2-metadata-token-ttl-seconds", "21600")],
                ..text(200, "controlled-imds-token")
            }
        }
        "/latest/meta-data/iam/security-credentials/" => {
            log.sequence.push("instance-role".to_owned());
            text(200, "controlled-role")
        }
        _ if path.starts_with("/latest/meta-data/iam/security-credentials/") => {
            log.sequence.push("instance".to_owned());
            let body = credentials("instance-key").replacen('{', r#"{"Code":"Success","#, 1);
            json(200, &body)
        }
        _ if path.starts_with("/federation/credentials") => {
            sso_credentials(input, request, query, log)
        }
        "/token" => {
            log.sequence.push("oidc".to_owned());
            json(400, r#"{"error":"invalid_grant"}"#)
        }
        "/bedrock" => {
            log.sequence.push("bedrock".to_owned());
            log.sent = Some(Sent {
                authorization: request.header("authorization").map(undated),
                security_token: request.header("x-amz-security-token").map(str::to_owned),
            });
            text(200, "")
        }
        _ => role_token(input, request, log),
    }
}

/// Start the services of a row; `log` receives what they see.
pub fn serve(input: &Input, log: &Arc<Mutex<Log>>) -> TestResult<Routed> {
    let (input, log) = (input.clone(), Arc::clone(log));
    serve_routed(move |request| {
        let mut log = log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        answer(&input, request, &mut log)
    })
}
