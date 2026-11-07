use std::convert::Infallible;
use std::io;
use std::sync::mpsc::{self, Sender};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use traojudge_workflow::payload::{Payload, Revision, Revision0, Upcast};
use traojudge_workflow::{JobId, JobStartCallback};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct InputV0 {
    executable: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct InputV1 {
    command: String,
    args: Vec<String>,
}

impl Upcast<InputV0> for InputV1 {
    type Error = Infallible;

    fn upcast(previous: InputV0) -> Result<Self, Self::Error> {
        Ok(Self {
            command: previous.executable,
            args: Vec::new(),
        })
    }
}

type InputSchema = Revision<InputV1, Revision0<InputV0>>;

struct DispatchCallback {
    sender: Sender<(JobId, InputV1)>,
}

#[async_trait]
impl JobStartCallback for DispatchCallback {
    type Input = InputSchema;
    type Error = io::Error;

    async fn start(&self, job_id: JobId, payload: Payload<Self::Input>) -> Result<(), Self::Error> {
        self.sender
            .send((job_id, payload.into_value()))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "job receiver closed"))
    }
}

#[tokio::test]
async fn callback_dispatches_typed_input_through_a_send_future_and_trait_object() {
    let (sender, receiver) = mpsc::channel();
    let callback: Box<dyn JobStartCallback<Input = InputSchema, Error = io::Error>> =
        Box::new(DispatchCallback { sender });
    let job_id = JobId::new();
    let payload = Payload::<InputSchema>::read(0, json!({ "executable": "compile" })).unwrap();

    tokio::spawn(async move { callback.start(job_id, payload).await })
        .await
        .unwrap()
        .unwrap();

    let (received_id, received_input) = receiver.try_recv().unwrap();
    assert_eq!(received_id, job_id);
    assert_eq!(
        received_input,
        InputV1 {
            command: "compile".to_owned(),
            args: Vec::new(),
        }
    );
}

#[tokio::test]
async fn callback_reports_dispatch_failure() {
    let (sender, receiver) = mpsc::channel();
    let callback = DispatchCallback { sender };
    drop(receiver);
    let payload = Payload::<InputSchema>::new(InputV1 {
        command: "compile".to_owned(),
        args: Vec::new(),
    })
    .unwrap();

    let error = callback.start(JobId::new(), payload).await.unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn job_id_round_trips_as_a_uuid_string() {
    let uuid = Uuid::from_u128(42);
    let job_id = JobId::from(uuid);

    let serialized = serde_json::to_value(job_id).unwrap();
    let restored: JobId = serde_json::from_value(serialized.clone()).unwrap();

    assert_eq!(serialized, json!(uuid.to_string()));
    assert_eq!(restored, job_id);
    assert_eq!(restored.as_uuid(), &uuid);
    assert_eq!(Uuid::from(restored), uuid);
}
