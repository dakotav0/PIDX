/// Field classes control how quickly observations lose confidence over time.
///
/// Each variant maps to a decay rate (λ) used in the formula:
///   effective_confidence = base_confidence × e^(−λ × days_since_observation)
///
/// A higher λ means faster decay. Identity traits decay almost imperceptibly;
/// signal phrases (idioms, phrasings) decay quickly because language patterns
/// shift faster than core personality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldClass {
    /// Core personality traits. Almost no decay (λ = 0.0005).
    Identity,
    /// Ethical preferences and constraints. Near-stable (λ = 0.0008).
    Value,
    /// Communication register evidence pool (λ = 0.0100).
    Register,
    /// Domain expertise clusters (λ = 0.0080).
    Domain,
    /// Collaboration style and pace (λ = 0.0070).
    Working,
    /// Idiomatic phrases, rhythms, avoidances. Fast decay (λ = 0.0200).
    Signal,
    /// User and system notes. Never decays (λ = 0.0).
    Annotation,
    /// NPC Stats. Decays very slowly (λ = 0.0002).
    NpcStat,
    /// NPC Identity (class, alignment). (λ = 0.0010).
    NpcIdentity,
    /// NPC Behavior derived/observed pool. (λ = 0.0150).
    NpcBehavior,
    /// NPC Skills. Fast decay (λ = 0.0200).
    NpcSkill,
    /// NPC Social/reputation traits. (λ = 0.0120).
    NpcSocial,
    /// NPC Memories. Very fast decay (λ = 0.0500).
    NpcMemory,
    /// NPC Working characteristics. (λ = 0.0100).
    NpcWorking,
}

impl FieldClass {
    /// Exponential decay rate λ for this field class.
    ///
    /// The compiler enforces exhaustiveness — add a new variant and `cargo build`
    /// will tell you to handle it here before you can ship.
    pub fn lambda(self) -> f64 {
        match self {
            FieldClass::Identity   => 0.0005,
            FieldClass::Value      => 0.0008,
            FieldClass::Register   => 0.0100,
            FieldClass::Domain     => 0.0080,
            FieldClass::Working    => 0.0070,
            FieldClass::Signal     => 0.0200,
            FieldClass::Annotation => 0.0,
            FieldClass::NpcStat    => 0.0002,
            FieldClass::NpcIdentity => 0.0010,
            FieldClass::NpcBehavior => 0.0150,
            FieldClass::NpcSkill    => 0.0200,
            FieldClass::NpcSocial   => 0.0120,
            FieldClass::NpcMemory   => 0.0500,
            FieldClass::NpcWorking  => 0.0100,
        }
    }
}

/// Compute the multiplicative decay factor for a given field class and age.
///
/// Returns a value in (0.0, 1.0] — multiply this by an observation's base
/// confidence to get its effective confidence at the time of `days` elapsed.
///
/// `days` should be non-negative. Negative values (future timestamps) return > 1.0,
/// which the caller should clamp if needed.
pub fn decay_factor(field_class: FieldClass, days: f64) -> f64 {
    (-field_class.lambda() * days).exp()
}
