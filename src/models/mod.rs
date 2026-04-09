// Public re-exports flatten the type hierarchy for downstream consumers.
// Allow unused-import warnings here — these are intentional API surface items,
// not all of which are used within the library itself.
#![allow(unused_imports)]

pub mod bridge;
pub mod confidence;
pub mod decay;
pub mod evidence;
pub mod miin_profile;
pub mod observation;
pub mod profile;

// Flatten the most-used types to `crate::models::Foo` so callers don't need
// to know which submodule each type lives in.
pub use confidence::{get_base_confidence, Origination, CORROBORATION_BONUS};
pub use decay::{decay_factor, FieldClass};
pub use evidence::{Evidence, EvidenceType, RegisterMetric, RegisterMetricName};
pub use miin_profile::{
    Alignment, MemoryRegister, MiinProfileDocument, NpcBehavior, NpcClass, NpcIdentity, NpcWorking,
    StatBlock, WorldbookRef,
};
pub use observation::{
    DomainEntry, Observation, ObservationField, ObservationSource, ObservationStatus,
    ObservationValue,
};
pub use profile::{
    Annotation, BridgeLog, BridgeLogEntry, CleanupPolicy, DeltaItem, Identity, IdentityReasoning,
    ProfileDocument, ProfileMeta, Register, ReviewItem, Signals, Working,
};
