use chrono::{NaiveDateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::decay::FieldClass;
use super::evidence::RegisterMetric;
use super::observation::{ObservationField, ObservationValue};
use super::profile::{Annotation, BridgeLog, DeltaItem, ProfileMeta, ReviewItem};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct StatBlock {
    #[serde(default)]
    pub str: ObservationField,
    #[serde(default)]
    pub dex: ObservationField,
    #[serde(default)]
    pub con: ObservationField,
    #[serde(default)]
    pub int: ObservationField,
    #[serde(default)]
    pub wis: ObservationField,
    #[serde(default)]
    pub cha: ObservationField,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Alignment {
    #[serde(default)]
    pub moral: ObservationField,
    #[serde(default)]
    pub order: ObservationField,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct NpcClass {
    #[serde(default)]
    pub primary: ObservationField,
    #[serde(default)]
    pub secondary: ObservationField,
    #[serde(default)]
    pub level: ObservationField,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct NpcIdentity {
    #[serde(default)]
    pub archetype: ObservationField,
    #[serde(default)]
    pub core: Vec<ObservationField>,
    #[serde(default)]
    pub stance: ObservationField,
    #[serde(default)]
    pub sub_archetype: ObservationField,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct NpcBehavior {
    #[serde(default)]
    pub aggression: RegisterMetric,
    #[serde(default)]
    pub sociability: RegisterMetric,
    #[serde(default)]
    pub curiosity: RegisterMetric,
    #[serde(default)]
    pub industriousness: RegisterMetric,
    #[serde(default)]
    pub caution: RegisterMetric,
    #[serde(default)]
    pub loyalty: RegisterMetric,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct WorldbookRef {
    pub worldbook_id: String,
    pub worldbook_path: String,
    #[serde(default)]
    pub setting_tags: Vec<String>,
    #[serde(default)]
    pub lore_injected: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct NpcWorking {
    #[serde(default)]
    pub active_period: ObservationField,
    #[serde(default)]
    pub range: ObservationField,
    #[serde(default)]
    pub engagement: ObservationField,
    #[serde(default)]
    pub quest_affinity: ObservationField,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct MemoryRegister {
    #[serde(default)]
    pub recent: Vec<ObservationField>,
    #[serde(default)]
    pub core: Vec<ObservationField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MiinProfileDocument {
    #[serde(default = "default_profile_type")]
    pub profile_type: String, // "miin"
    pub meta: ProfileMeta,
    #[serde(default)]
    pub stats: StatBlock,
    #[serde(default)]
    pub alignment: Alignment,
    #[serde(default)]
    pub class: NpcClass,
    #[serde(default)]
    pub identity: NpcIdentity,
    #[serde(default)]
    pub behavior: NpcBehavior,
    #[serde(default)]
    pub skills: Vec<ObservationField>,
    #[serde(default)]
    pub social: Vec<ObservationField>,
    #[serde(default)]
    pub memories: MemoryRegister,
    #[serde(default)]
    pub working: NpcWorking,
    #[serde(default)]
    pub worldbook: Option<WorldbookRef>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    #[serde(default)]
    pub delta_queue: Vec<DeltaItem>,
    #[serde(default)]
    pub review_queue: Vec<ReviewItem>,
    #[serde(default)]
    pub bridge_log: BridgeLog,
}

fn default_profile_type() -> String {
    "miin".to_string()
}

impl MiinProfileDocument {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            profile_type: "miin".to_string(),
            meta: ProfileMeta::new(id),
            stats: StatBlock::default(),
            alignment: Alignment::default(),
            class: NpcClass::default(),
            identity: NpcIdentity::default(),
            behavior: NpcBehavior::default(),
            skills: Vec::new(),
            social: Vec::new(),
            memories: MemoryRegister::default(),
            working: NpcWorking::default(),
            worldbook: None,
            annotations: Vec::new(),
            delta_queue: Vec::new(),
            review_queue: Vec::new(),
            bridge_log: BridgeLog::default(),
        }
    }

    pub fn recompute_overall_confidence(&mut self) {
        // Simple average of confidence across all identity fields
        let mut sum = 0.0;
        let mut count = 0;
        for f in [&self.class.primary, &self.class.secondary] {
            sum += f.overall_confidence();
            count += 1;
        }
        if count > 0 {
            self.meta.overall_confidence = sum / count as f64;
        }
    }

    pub fn recompute_derived_behaviors(&mut self) {
        // This is a logic placeholder. In a more complex version,
        // we might cache the derived results or check for drift here.
        self.recompute_overall_confidence();
    }

    /// Determines the active mRNA adapter for routing.
    /// Requires high confidence and dense memory to authorize a sub-archetype transfer.
    pub fn get_active_adapter(&self) -> String {
        // 1. Get the foundational baseline adapter
        let base_archetype = if let Some(ObservationValue::Text(v)) =
            self.identity.archetype.active(FieldClass::NpcIdentity)
        {
            v.clone().to_lowercase()
        } else {
            "merchant".to_string() // Cold-start fallback
        };

        // 2. Check if a sub-archetype has been proposed/confirmed
        let sub_archetype_opt = self.identity.sub_archetype.active(FieldClass::NpcIdentity);

        if let Some(ObservationValue::Text(sub_arch)) = sub_archetype_opt {
            // 3. The Quality Gates
            let has_dense_memory = self.memories.core.len() >= 5;
            let has_high_confidence = self.meta.overall_confidence > 0.85;

            // 4. Threshold Check
            if has_dense_memory && has_high_confidence {
                return sub_arch.clone().to_lowercase();
            }
        }

        // 5. Fallback to base if thresholds aren't met
        base_archetype
    }
}

pub fn get_stat_value(field: &ObservationField) -> f64 {
    if let Some(ObservationValue::Number(v)) = field.active(FieldClass::NpcStat) {
        *v
    } else {
        10.0 // Default fallback stat
    }
}

pub fn get_alignment_value(field: &ObservationField) -> String {
    if let Some(ObservationValue::Text(v)) = field.active(FieldClass::NpcIdentity) {
        v.clone().to_lowercase()
    } else {
        "neutral".to_string()
    }
}

fn norm(stat: f64) -> f64 {
    let stat = stat.clamp(1.0, 20.0);
    (stat - 1.0) / 19.0
}

fn align_mod(axis: &str, positive: &str, negative: &str) -> f64 {
    if axis == positive {
        0.15
    } else if axis == negative {
        -0.15
    } else {
        0.0
    }
}

impl MiinProfileDocument {
    pub fn derive_aggression(&self) -> f64 {
        let str = get_stat_value(&self.stats.str);
        let dex = get_stat_value(&self.stats.dex);
        let order = get_alignment_value(&self.alignment.order);
        let base = norm(str) * 0.6 + norm(dex) * 0.2;
        let m = align_mod(&order, "chaotic", "lawful");
        (base + m).clamp(0.0, 1.0)
    }

    pub fn derive_sociability(&self) -> f64 {
        let cha = get_stat_value(&self.stats.cha);
        let wis = get_stat_value(&self.stats.wis);
        let moral = get_alignment_value(&self.alignment.moral);
        let base = norm(cha) * 0.7 + norm(wis) * 0.2;
        let m = align_mod(&moral, "good", "evil");
        (base + m).clamp(0.0, 1.0)
    }

    pub fn derive_curiosity(&self) -> f64 {
        let int = get_stat_value(&self.stats.int);
        let wis = get_stat_value(&self.stats.wis);
        let cha = get_stat_value(&self.stats.cha);
        let order = get_alignment_value(&self.alignment.order);
        let base = norm(int) * 0.5 + norm(wis) * 0.3 + norm(cha) * 0.1;
        let m = align_mod(&order, "chaotic", "lawful") * 0.5;
        (base + m).clamp(0.0, 1.0)
    }

    pub fn derive_industriousness(&self) -> f64 {
        let con = get_stat_value(&self.stats.con);
        let int = get_stat_value(&self.stats.int);
        let wis = get_stat_value(&self.stats.wis);
        let order = get_alignment_value(&self.alignment.order);
        let base = norm(con) * 0.5 + norm(int) * 0.2 + norm(wis) * 0.2;
        let m = align_mod(&order, "lawful", "chaotic");
        (base + m).clamp(0.0, 1.0)
    }

    pub fn derive_caution(&self) -> f64 {
        let wis = get_stat_value(&self.stats.wis);
        let int = get_stat_value(&self.stats.int);
        let str = get_stat_value(&self.stats.str);
        let order = get_alignment_value(&self.alignment.order);
        let base = norm(wis) * 0.6 + norm(int) * 0.2;
        let penalty = norm(str) * -0.15;
        let m = align_mod(&order, "lawful", "chaotic") * 0.5;
        (base + penalty + m).clamp(0.0, 1.0)
    }

    pub fn derive_loyalty(&self) -> f64 {
        let cha = get_stat_value(&self.stats.cha);
        let wis = get_stat_value(&self.stats.wis);
        let moral = get_alignment_value(&self.alignment.moral);
        let order = get_alignment_value(&self.alignment.order);
        let base = norm(cha) * 0.3 + norm(wis) * 0.2;
        let moral_mod = align_mod(&moral, "good", "evil");
        let order_mod = align_mod(&order, "lawful", "chaotic");
        (base + moral_mod + order_mod).clamp(0.0, 1.0)
    }

    pub fn effective_behavior(&self, metric: &str, as_of: Option<chrono::DateTime<Utc>>) -> f64 {
        let (derived, evs) = match metric {
            "aggression" => (self.derive_aggression(), &self.behavior.aggression.evidence),
            "sociability" => (
                self.derive_sociability(),
                &self.behavior.sociability.evidence,
            ),
            "curiosity" => (self.derive_curiosity(), &self.behavior.curiosity.evidence),
            "industriousness" => (
                self.derive_industriousness(),
                &self.behavior.industriousness.evidence,
            ),
            "caution" => (self.derive_caution(), &self.behavior.caution.evidence),
            "loyalty" => (self.derive_loyalty(), &self.behavior.loyalty.evidence),
            _ => return 0.0,
        };

        if evs.is_empty() {
            return derived;
        }

        let as_of = as_of.unwrap_or_else(Utc::now);
        let lam = FieldClass::NpcBehavior.lambda();

        let mut total_weight = 0.0_f64;
        let mut weighted_signal = 0.0_f64;

        for e in evs {
            let decay = if e.decay_exempt {
                1.0
            } else {
                match NaiveDateTime::parse_from_str(&e.observed_at, "%Y-%m-%dT%H:%M:%S%.f") {
                    Ok(t) => {
                        let days = (as_of - t.and_utc()).num_seconds() as f64 / 86400.0;
                        (-lam * days).exp()
                    }
                    Err(_) => 1.0,
                }
            };
            let w = e.weight * decay;
            total_weight += w;
            weighted_signal += e.signal * w;
        }

        let observed_score = if total_weight > 0.0 {
            let raw = weighted_signal / total_weight; // [-1.0, 1.0]
            (raw + 1.0) / 2.0 // Map to [0.0, 1.0]
        } else {
            0.5
        };

        let observed_weight = (evs.len() as f64 / 20.0).min(0.8);
        derived * (1.0 - observed_weight) + observed_score * observed_weight
    }
}
