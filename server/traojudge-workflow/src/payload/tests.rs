use std::convert::Infallible;
use std::error::Error;
use std::io;

use serde_json::json;

use super::*;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ValueV0 {
    text: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ValueV1 {
    message: String,
    priority: u32,
}

impl Upcast<ValueV0> for ValueV1 {
    type Error = Infallible;

    fn upcast(previous: ValueV0) -> Result<Self, Self::Error> {
        Ok(Self {
            message: previous.text,
            priority: 1,
        })
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ValueV2 {
    message: String,
    priority: u32,
    tags: Vec<String>,
}

impl Upcast<ValueV1> for ValueV2 {
    type Error = io::Error;

    fn upcast(previous: ValueV1) -> Result<Self, Self::Error> {
        if previous.message.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty message"));
        }
        Ok(Self {
            message: previous.message,
            priority: previous.priority,
            tags: vec!["workflow".to_owned()],
        })
    }
}

type Schema0 = Revision0<ValueV0>;
type Schema1 = Revision<ValueV1, Schema0>;
type Schema2 = Revision<ValueV2, Schema1>;

#[test]
fn initial_payload_keeps_value_json_and_revision_together() {
    let payload = Payload::<Schema0>::new(ValueV0 {
        text: "hello".to_owned(),
    })
    .unwrap();

    assert_eq!(payload.revision(), 0);
    assert_eq!(payload.json(), &json!({ "text": "hello" }));
    assert_eq!(payload.value().text, "hello");
    assert_eq!(payload.into_value().text, "hello");
}

#[test]
fn revisions_increment_from_zero() {
    assert_eq!(Schema0::REVISION, 0);
    assert_eq!(Schema1::REVISION, 1);
    assert_eq!(Schema2::REVISION, 2);
}

#[test]
fn revision_zero_reads_without_an_upcast_implementation() {
    let payload = Payload::<Schema0>::read(0, json!({ "text": "hello" })).unwrap();

    assert_eq!(payload.value().text, "hello");
    assert_eq!(payload.revision(), 0);
}

#[test]
fn read_upcasts_each_intermediate_generation() {
    let payload = Payload::<Schema2>::read(0, json!({ "text": "hello" })).unwrap();

    assert_eq!(payload.revision(), 2);
    assert_eq!(payload.value().message, "hello");
    assert_eq!(payload.value().priority, 1);
    assert_eq!(payload.value().tags, ["workflow"]);
    assert_eq!(
        payload.json(),
        &json!({ "message": "hello", "priority": 1, "tags": ["workflow"] })
    );
}

#[test]
fn read_can_start_at_an_intermediate_generation() {
    let payload =
        Payload::<Schema2>::read(1, json!({ "message": "hello", "priority": 7 })).unwrap();

    assert_eq!(payload.value().priority, 7);
    assert_eq!(payload.value().tags, ["workflow"]);
}

#[test]
fn current_generation_does_not_run_upcasts() {
    let stored = json!({ "message": "", "priority": 7, "tags": ["custom"] });
    let payload = Payload::<Schema2>::read(2, stored.clone()).unwrap();

    assert_eq!(payload.json(), &stored);
    assert_eq!(payload.value().tags, ["custom"]);
}

#[test]
fn serde_round_trip_serializes_revision_and_json() {
    let payload = Payload::<Schema1>::new(ValueV1 {
        message: "hello".to_owned(),
        priority: 3,
    })
    .unwrap();

    let encoded = serde_json::to_string(&payload).unwrap();
    let decoded: Payload<Schema1> = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.value(), payload.value());
    assert_eq!(decoded.json(), payload.json());
    assert_eq!(decoded.revision(), 1);
    assert_eq!(
        serde_json::from_str::<Value>(&encoded).unwrap(),
        json!({ "revision": 1, "json": { "message": "hello", "priority": 3 } })
    );
}

#[test]
fn serde_reads_old_payload_and_writes_latest_generation() {
    let payload: Payload<Schema2> = serde_json::from_value(json!({
        "revision": 0,
        "json": { "text": "hello" }
    }))
    .unwrap();

    assert_eq!(
        serde_json::to_value(payload).unwrap(),
        json!({
            "revision": 2,
            "json": { "message": "hello", "priority": 1, "tags": ["workflow"] }
        })
    );
}

#[test]
fn future_revisions_are_rejected_even_if_json_matches() {
    assert!(matches!(
        Payload::<Schema0>::read(1, json!({ "text": "hello" })),
        Err(PayloadError::UnsupportedRevision {
            revision: 1,
            latest_revision: 0
        })
    ));
    assert!(matches!(
        Payload::<Schema2>::read(3, json!({ "message": "hello", "priority": 1, "tags": [] })),
        Err(PayloadError::UnsupportedRevision {
            revision: 3,
            latest_revision: 2
        })
    ));
}

#[test]
fn invalid_json_reports_the_stored_revision_without_fallback() {
    for revision in 0..=2 {
        assert!(matches!(
            Payload::<Schema2>::read(revision, json!({})),
            Err(PayloadError::Deserialize { revision: failed, .. }) if failed == revision
        ));
    }
}

#[test]
fn upcast_errors_report_the_failing_transition_and_preserve_the_source() {
    let error = match Payload::<Schema2>::read(0, json!({ "text": "" })) {
        Err(error) => error,
        Ok(_) => panic!("empty message should fail upcasting"),
    };

    assert!(matches!(
        error,
        PayloadError::Upcast {
            from_revision: 1,
            to_revision: 2,
            ..
        }
    ));
    let source = error.source().unwrap().downcast_ref::<io::Error>().unwrap();
    assert_eq!(source.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn malformed_envelopes_are_rejected() {
    for stored in [
        json!({ "json": { "text": "hello" } }),
        json!({ "revision": 0 }),
        json!({ "revision": -1, "json": { "text": "hello" } }),
        json!({ "revision": 0.5, "json": { "text": "hello" } }),
        json!({ "revision": 4294967296_u64, "json": { "text": "hello" } }),
        json!({ "revision": "0", "json": { "text": "hello" } }),
    ] {
        assert!(serde_json::from_value::<Payload<Schema0>>(stored).is_err());
    }
}

#[derive(Deserialize)]
struct Unserializable;

impl Serialize for Unserializable {
    fn serialize<Output: Serializer>(&self, _: Output) -> Result<Output::Ok, Output::Error> {
        Err(serde::ser::Error::custom("serialization failed"))
    }
}

#[test]
fn serialization_errors_do_not_construct_inconsistent_payloads() {
    assert!(matches!(
        Payload::<Revision0<Unserializable>>::new(Unserializable),
        Err(PayloadError::Serialize { revision: 0, .. })
    ));
    assert!(matches!(
        Payload::<Revision0<Unserializable>>::read(0, Value::Null),
        Err(PayloadError::Serialize { revision: 0, .. })
    ));
}

#[test]
fn json_is_normalized_to_the_typed_value() {
    let payload = Payload::<Schema0>::read(0, json!({ "text": "hello", "extra": true })).unwrap();

    assert_eq!(payload.json(), &json!({ "text": "hello" }));
}
