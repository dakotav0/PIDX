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
