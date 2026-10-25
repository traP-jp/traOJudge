use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::PayloadError;

pub trait Upcast<Previous>: Sized {
    type Error: std::error::Error + Send + Sync + 'static;

    fn upcast(previous: Previous) -> Result<Self, Self::Error>;
}

pub trait PayloadRevision {
    type Value: Serialize + DeserializeOwned;

    const REVISION: u32;

    fn read(revision: u32, json: Value) -> Result<Self::Value, PayloadError>;
}

pub struct Revision0<Current>(PhantomData<fn() -> Current>);

impl<Current: Serialize + DeserializeOwned> PayloadRevision for Revision0<Current> {
    type Value = Current;

    const REVISION: u32 = 0;

    fn read(revision: u32, json: Value) -> Result<Self::Value, PayloadError> {
        if revision != Self::REVISION {
            return Err(PayloadError::UnsupportedRevision {
                revision,
                latest_revision: Self::REVISION,
            });
        }
        serde_json::from_value(json)
            .map_err(|source| PayloadError::Deserialize { revision, source })
    }
}

pub struct Revision<Current, Previous>(PhantomData<fn() -> (Current, Previous)>);

impl<Current, Previous> PayloadRevision for Revision<Current, Previous>
where
    Previous: PayloadRevision,
    Current: Serialize + DeserializeOwned + Upcast<Previous::Value>,
{
    type Value = Current;

    const REVISION: u32 = Previous::REVISION + 1;

    fn read(revision: u32, json: Value) -> Result<Self::Value, PayloadError> {
        if revision > Self::REVISION {
            return Err(PayloadError::UnsupportedRevision {
                revision,
                latest_revision: Self::REVISION,
            });
        }
        if revision == Self::REVISION {
            return serde_json::from_value(json)
                .map_err(|source| PayloadError::Deserialize { revision, source });
        }

        let previous = Previous::read(revision, json)?;
        Current::upcast(previous).map_err(|source| PayloadError::Upcast {
            from_revision: Previous::REVISION,
            to_revision: Self::REVISION,
            source: Box::new(source),
        })
    }
}
