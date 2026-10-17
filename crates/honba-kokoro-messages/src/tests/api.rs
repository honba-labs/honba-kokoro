use serde_json::json;

use crate::{API_VERSION, ApiResponse, ErrorDetail, ResponseEnvelope};

#[test]
fn success_carries_data_and_no_error() {
    let env: ResponseEnvelope<serde_json::Value> = ApiResponse::success(json!({"ok": true}));
    assert_eq!(env.api_version.as_str(), API_VERSION);
    assert_eq!(env.schema_version, crate::SCHEMA_VERSION);
    assert!(env.error.is_none());
    assert!(!env.is_error());
    assert_eq!(env.data.unwrap()["ok"], json!(true));
}

#[test]
fn a_failing_envelope_round_trips_its_stable_code() {
    let env: ResponseEnvelope<serde_json::Value> =
        ApiResponse::error(ErrorDetail::new(crate::ErrorCode::NotFound, "gone"));
    let text = serde_json::to_string(&env).unwrap();
    let back: ResponseEnvelope<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert_eq!(
        back.error
            .expect("error detail survives the round trip")
            .code,
        crate::ErrorCode::NotFound
    );
}

#[test]
fn into_result_unwraps_a_success() {
    let env: ResponseEnvelope<u8> = ApiResponse::success(7);
    assert_eq!(env.into_result().unwrap(), 7);
}

#[test]
fn error_serializes_with_a_null_data_key_because_it_is_absent() {
    let env: ResponseEnvelope<serde_json::Value> =
        ApiResponse::error(ErrorDetail::new(crate::ErrorCode::Timeout, "slow"));
    let v = serde_json::to_value(&env).unwrap();
    assert!(v.get("data").is_none());
    assert_eq!(v["error"]["code"], json!("timeout"));
    assert_eq!(v["error"]["retryable"], json!(true));
}

#[test]
fn an_unknown_optional_field_from_a_newer_writer_is_ignored() {
    // docs/archive/plan.md 4.2: readers must ignore unknown fields so a v1 reader can
    // consume v1-with-extras safely.
    let raw = json!({
        "api_version": API_VERSION,
        "schema_version": crate::SCHEMA_VERSION,
        "data": {"ok": true},
        "some_future_field": {"added": "later"},
    });
    let env: ResponseEnvelope<serde_json::Value> =
        serde_json::from_value(raw).expect("unknown field must be ignored");
    assert_eq!(env.data.unwrap()["ok"], json!(true));
}
