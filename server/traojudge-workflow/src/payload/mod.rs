use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

mod revision;

pub use revision::{PayloadRevision, Revision, Revision0, Upcast};

pub struct Payload<Schema: PayloadRevision> {
    value: Schema::Value,
    json: Value,
}

impl<Schema: PayloadRevision> Payload<Schema> {
    pub fn new(value: Schema::Value) -> Result<Self, PayloadError> {
        let json = serde_json::to_value(&value).map_err(|source| PayloadError::Serialize {
            revision: Schema::REVISION,
            source,
        })?;
        Ok(Self { value, json })
    }

    pub fn read(revision: u32, json: Value) -> Result<Self, PayloadError> {
        Self::new(Schema::read(revision, json)?)
    }

    pub fn value(&self) -> &Schema::Value {
        &self.value
    }

    pub fn json(&self) -> &Value {
        &self.json
    }

    pub fn revision(&self) -> u32 {
        Schema::REVISION
    }

    pub fn into_value(self) -> Schema::Value {
        self.value
    }
}

impl<Schema: PayloadRevision> Serialize for Payload<Schema> {
    fn serialize<Output: Serializer>(
        &self,
        serializer: Output,
    ) -> Result<Output::Ok, Output::Error> {
        let mut state = serializer.serialize_struct("Payload", 2)?;
        state.serialize_field("revision", &self.revision())?;
        state.serialize_field("json", &self.json)?;
        state.end()
    }
}

impl<'de, Schema: PayloadRevision> Deserialize<'de> for Payload<Schema> {
    fn deserialize<Input: Deserializer<'de>>(deserializer: Input) -> Result<Self, Input::Error> {
        let stored = StoredPayload::deserialize(deserializer)?;
        Self::read(stored.revision, stored.json).map_err(serde::de::Error::custom)
    }
}

#[derive(Deserialize)]
struct StoredPayload {
    revision: u32,
    json: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum PayloadError {
    #[error("payload revision {revision} is newer than supported revision {latest_revision}")]
    UnsupportedRevision { revision: u32, latest_revision: u32 },
    #[error("failed to deserialize payload revision {revision}: {source}")]
    Deserialize {
        revision: u32,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize payload revision {revision}: {source}")]
    Serialize {
        revision: u32,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to upcast payload revision {from_revision} to {to_revision}: {source}")]
    Upcast {
        from_revision: u32,
        to_revision: u32,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync + 'static>,
    },
}

#[cfg(test)]
mod tests;
