use std::fmt::Debug;
use anyhow::{bail, Result};
use chrono::Utc;
use cloud_util::InstanceId;
use crate::common::response::{error_response, success_response};
use crate::model::request::{GetAssignmentRequest, PutAssignmentRequest};
use crate::service::discovery_service;
use crate::service::discovery_service::Deps;
use lambda_http::{Body, Request, RequestExt, Response};
use tracing::{error, instrument, warn, Span};
use crate::model::body::PutAssignmentBody;

#[instrument(skip(deps), fields(instance_id = tracing::field::Empty))]
pub async fn handle_get(req: Request, deps: Deps) -> Response<Body> {
    let instance_id = match parse_instance_id(&req) {
        Ok(id) => id,
        Err(e) => return error_response(400, e)
    };

    Span::current().record("instance_id", tracing::field::display(&instance_id));

    let get_assignment_request = GetAssignmentRequest { instance_id };

    match discovery_service::get_assignment(get_assignment_request, deps).await {
        Ok(assignment) => success_response(Some(assignment)),
        Err(e) => handle_cloud_error("Failed to fetch instance assignment", &e),
    }
}

#[instrument(skip(deps), fields(instance_id = tracing::field::Empty))]
pub async fn handle_put(req: Request, deps: Deps) -> Response<Body> {
    let instance_id = match parse_instance_id(&req) {
        Ok(id) => id,
        Err(e) => return error_response(400, e)
    };

    Span::current().record("instance_id", tracing::field::display(&instance_id));

    let body: PutAssignmentBody = match serde_json::from_slice(req.body().as_ref()) {
        Ok(b) => b,
        Err(e) => {
            warn!(error = ?e, "Failed to parse request body as JSON");
            return error_response(400, "Invalid request body");
        }
    };
    
    if body.expire_at < Utc::now() {
        return error_response(400, "Assignment already expired");
    }

    let put_request = PutAssignmentRequest {
        instance_id,
        stocks: body.stocks,
        input: body.input,
        output: body.output,
        expire_at: body.expire_at,
    };

    match discovery_service::put_assignment(put_request, deps).await {
        Ok(assignment) => success_response(Some(assignment)),
        Err(e) => handle_cloud_error("Failed to persist instance assignment", &e)
    }
}

fn handle_cloud_error(context: &str, err: impl Debug) -> Response<Body> {
    let msg = format!("{}: {:?}", context, err);
    error!(msg);
    error_response(400, msg)
}

fn parse_instance_id(req: &Request) -> Result<InstanceId> {
    let id_str = match req.path_parameters().first("id") {
        Some(id) => id.to_string(),
        None => {
            warn!(path = %req.uri().path(), "Missing path parameter: `id`");
            bail!("Missing path parameter: id")
        }
    };

    match InstanceId::new(id_str.clone()) {
        Ok(id) => Ok(id),
        Err(e) => {
            warn!(invalid_id = %id_str, "Failed to parse instance ID");
            bail!("Invalid instance ID: {:?}", e)
        }
    }
}
