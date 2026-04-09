import { useState, useEffect, useRef } from "react";

// ── TOKENS ──────────────────────────────────────────────────────────────────
const T = {
  bg:       "#0e0f0d",
  surface:  "#161714",
  lift:     "#1d1f1b",
  border:   "#252720",
  borderHi: "#3a3d35",
  text:     "#d4cfc7",
  muted:    "#7a786f",
  faint:    "#3a3830",
  sage:     "#7fa88a",
  sageD:    "#4a6b56",
  sageL:    "#a8c9b4",
  gold:     "#c9a96e",
  goldD:    "#8a7040",
  cream:    "#ede8df",
};

// ── SAMPLE DATA ─────────────────────────────────────────────────────────────
const PROFILE = {
  meta: { id: "usr_d4k0t4", version: "0.3.1", confidence: 0.88, updated: "2026-04-06" },
  core: [
    "systems architect with humanist instincts",
    "aesthetic-first, structure-second thinker",
    "values-aligned builder — actively avoids extractive tech",
  ],
  register: {
    formality:   { score: 2.1, label: "Casual"  },
    directness:  { score: 7.8, label: "Direct"  },
    hedging:     { score: 2.9, label: "Low"     },
    humor:       { score: 6.3, label: "Dry/Wry" },
    abstraction: { score: 7.1, label: "High"    },
  },
  domains: [
    { label: "Indigenous language tech", weight: 0.94 },
    { label: "Systems architecture",     weight: 0.89 },
    { label: "Full-stack development",   weight: 0.84 },
    { label: "Political theory",         weight: 0.71 },
    { label: "Creative writing / poetry",weight: 0.68 },
  ],
  values: ["data sovereignty","community over individual gain","process transparency","Indigenous self-determination"],
  working: { mode: "Co-creator, not client", pace: "Async-friendly, burst-capable", feedback: "Direct critique preferred" },
  deltas: 2,
  review: 1,
};

const SESSION_OBSERVATIONS = [
  {
    id: 1,
    type: "signal",
    field: "signals.phrases",
    engine: `You used "topography" three times today as a stand-in for structure and shape. It holds — spatial metaphors consistently precede technical framing in how you think through problems.`,
    value: "topography (as structural metaphor)",
    status: "pending",
  },
  {
    id: 2,
    type: "ironic_hedging",
    field: "signals.phrases + humor",
    engine: `"micro project (I say that now)" — flagged as ironic understatement, not genuine uncertainty. You knew the scope the moment you said it. Routed to humor evidence, not hedging.`,
    value: "micro project (I say that now)",
    status: "pending",
  },
  {
    id: 3,
    type: "working",
    field: "working.mode",
    engine: `Sketch-level prompts throughout — "let's draft a mockup," "ready for the SKILL.md," no spec attached. You consistently hand off the structural decisions and engage the output. Co-creator pattern holds.`,
    value: "sketch-level prompt, expects extrapolation",
    status: "pending",
  },
  {
    id: 4,
    type: "domain",
    field: "domains",
    engine: `Schema design and skill architecture surfaced as active working domains today — distinct from the Niigaane/language-tech cluster. May warrant a separate domain entry or grouping.`,
    value: "schema design / skill architecture",
    status: "pending",
  },
];

const STABLE = [
  { field: "identity.core[2]", value: "values-aligned builder — actively avoids extractive tech", since: "14 sessions", confidence: 0.97 },
  { field: "working.feedback",  value: "direct critique preferred; no softening needed",            since: "9 sessions",  confidence: 0.94 },
  { field: "signals.phrases",   value: "\"viber\" as self-description",                             since: "6 sessions",  confidence: 0.91 },
];

const DELTAS = [
  {
    field: "domains",
    a: { value: "mRNA architecture", orientation: "local:gemma3:4b", conf: 0.61 },
    b: { value: "mRNA architecture", orientation: "claude.sonnet-4-6", conf: 0.78 },
    note: "Both orientations agree on the label but this domain hasn't appeared in recent sessions. Review or solidify?",
    compatible: true,
  },
];

// ── HELPERS ─────────────────────────────────────────────────────────────────
function Dot({ on, pulse }) {
  return (
    <span style={{
      display: "inline-block", width: 6, height: 6, borderRadius: "50%",
      background: on ? T.sage : T.faint, flexShrink: 0,
      animation: pulse ? "breathe 3s ease-in-out infinite" : "none",
    }} />
  );
}

function BarThin({ value, max = 10, color = T.sage }) {
  return (
    <div style={{ background: T.faint, borderRadius: 1, height: 2, width: "100%", overflow: "hidden" }}>
      <div style={{ width: `${(value / max) * 100}%`, height: "100%", background: color, borderRadius: 1, transition: "width 0.8s cubic-bezier(0.4,0,0.2,1)" }} />
    </div>
  );
}

function Badge({ children, color = T.sage }) {
  return (
    <span style={{
      fontSize: 9, fontFamily: "monospace", letterSpacing: "0.08em",
      padding: "2px 6px", borderRadius: 2,
      background: color + "18", color, border: `1px solid ${color}30`,
    }}>{children}</span>
  );
}

function Rule({ label }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 10, margin: "20px 0 12px" }}>
      <span style={{ height: 1, flex: 1, background: T.border }} />
      <span style={{ fontSize: 8, fontFamily: "monospace", letterSpacing: "0.14em", color: T.faint, textTransform: "uppercase" }}>{label}</span>
      <span style={{ height: 1, flex: 1, background: T.border }} />
    </div>
  );
}

// ── COMPACT MODE ─────────────────────────────────────────────────────────────
function CompactPanel({ onExpand, onCheckin }) {
  const [traitIdx, setTraitIdx] = useState(0);
  useEffect(() => {
    const t = setInterval(() => setTraitIdx(i => (i + 1) % PROFILE.core.length), 3800);
    return () => clearInterval(t);
  }, []);

  const confPct = PROFILE.meta.confidence * 100;

  return (
    <div style={{
      width: 300, background: T.surface, border: `1px solid ${T.border}`,
      borderRadius: 8, overflow: "hidden", fontFamily: "monospace",
    }}>
      {/* Header strip */}
      <div style={{ background: T.bg, padding: "12px 14px", display: "flex", alignItems: "center", justifyContent: "space-between", borderBottom: `1px solid ${T.border}` }}>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <Dot on pulse />
          <span style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 13, color: T.cream, letterSpacing: "-0.01em" }}>personality<span style={{ color: T.sage }}>·</span>idx</span>
        </div>
        <span style={{ fontSize: 9, color: T.muted }}>{PROFILE.meta.id}</span>
      </div>

      {/* Confidence ring area */}
      <div style={{ padding: "16px 14px 12px", borderBottom: `1px solid ${T.border}` }}>
        <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 6 }}>
          <span style={{ fontSize: 9, color: T.muted, letterSpacing: "0.1em" }}>OVERALL CONF</span>
          <span style={{ fontSize: 9, color: T.sage }}>{confPct.toFixed(0)}%</span>
        </div>
        <BarThin value={PROFILE.meta.confidence * 10} color={T.sage} />
      </div>

      {/* Rotating core trait */}
      <div style={{ padding: "14px", borderBottom: `1px solid ${T.border}`, minHeight: 62 }}>
        <div style={{ fontSize: 9, color: T.muted, letterSpacing: "0.1em", marginBottom: 8 }}>CORE · {traitIdx + 1}/{PROFILE.core.length}</div>
        <div style={{ fontSize: 12, color: T.text, lineHeight: 1.5, fontFamily: "Fraunces, Georgia, serif", fontStyle: "italic", transition: "opacity 0.4s" }}>
          {PROFILE.core[traitIdx]}
        </div>
      </div>

      {/* Pending indicators */}
      <div style={{ padding: "10px 14px", display: "flex", gap: 12, borderBottom: `1px solid ${T.border}` }}>
        <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <span style={{ width: 5, height: 5, borderRadius: "50%", background: PROFILE.deltas > 0 ? T.gold : T.faint, display: "inline-block" }} />
          <span style={{ fontSize: 9, color: PROFILE.deltas > 0 ? T.gold : T.muted }}>{PROFILE.deltas} delta{PROFILE.deltas !== 1 ? "s" : ""}</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <span style={{ width: 5, height: 5, borderRadius: "50%", background: PROFILE.review > 0 ? T.sageL : T.faint, display: "inline-block" }} />
          <span style={{ fontSize: 9, color: PROFILE.review > 0 ? T.sageL : T.muted }}>{PROFILE.review} for review</span>
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
          <span style={{ fontSize: 9, color: T.muted }}>{SESSION_OBSERVATIONS.length} new this session</span>
        </div>
      </div>

      {/* Actions */}
      <div style={{ padding: "10px 14px", display: "flex", gap: 8 }}>
        <button onClick={onCheckin} style={{
          flex: 1, padding: "7px 0", fontSize: 9, letterSpacing: "0.1em",
          background: T.sage + "18", border: `1px solid ${T.sage}40`, borderRadius: 4,
          color: T.sage, cursor: "pointer", fontFamily: "monospace",
        }}>CHECK IN</button>
        <button onClick={onExpand} style={{
          flex: 1, padding: "7px 0", fontSize: 9, letterSpacing: "0.1em",
          background: T.lift, border: `1px solid ${T.border}`, borderRadius: 4,
          color: T.muted, cursor: "pointer", fontFamily: "monospace",
        }}>FULL VIEW</button>
      </div>
    </div>
  );
}

// ── VISUAL MODE ──────────────────────────────────────────────────────────────
function VisualPanel({ onCheckin, onCompact }) {
  return (
    <div style={{ width: "100%", maxWidth: 680, fontFamily: "monospace" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 20, padding: "0 2px" }}>
        <div>
          <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 20, color: T.cream, letterSpacing: "-0.02em" }}>
            personality<span style={{ color: T.sage }}>·</span>idx
          </div>
          <div style={{ fontSize: 9, color: T.muted, marginTop: 2, letterSpacing: "0.1em" }}>
            {PROFILE.meta.id} · v{PROFILE.meta.version} · conf {(PROFILE.meta.confidence * 100).toFixed(0)}%
          </div>
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          <button onClick={onCheckin} style={{ fontSize: 9, padding: "5px 12px", background: T.sage + "18", border: `1px solid ${T.sage}40`, borderRadius: 4, color: T.sage, cursor: "pointer", fontFamily: "monospace", letterSpacing: "0.08em" }}>CHECK IN</button>
          <button onClick={onCompact} style={{ fontSize: 9, padding: "5px 12px", background: T.lift, border: `1px solid ${T.border}`, borderRadius: 4, color: T.muted, cursor: "pointer", fontFamily: "monospace", letterSpacing: "0.08em" }}>COMPACT</button>
        </div>
      </div>

      {/* Core */}
      <Rule label="core identity" />
      {PROFILE.core.map((t, i) => (
        <div key={i} style={{ display: "flex", gap: 10, marginBottom: 10, alignItems: "flex-start" }}>
          <Dot on />
          <span style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 13, color: T.text, lineHeight: 1.6, fontStyle: "italic" }}>{t}</span>
        </div>
      ))}

      {/* Register */}
      <Rule label="communication register" />
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px 20px" }}>
        {Object.entries(PROFILE.register).map(([k, v]) => (
          <div key={k}>
            <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 5 }}>
              <span style={{ fontSize: 9, color: T.muted, letterSpacing: "0.1em", textTransform: "uppercase" }}>{k}</span>
              <span style={{ fontSize: 9, color: T.sage }}>{v.label}</span>
            </div>
            <BarThin value={v.score} color={T.sage} />
          </div>
        ))}
      </div>

      {/* Domains */}
      <Rule label="domain clusters" />
      {PROFILE.domains.map((d, i) => (
        <div key={i} style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 9 }}>
          <span style={{ fontSize: 9, color: T.muted, minWidth: 32, textAlign: "right" }}>{Math.round(d.weight * 100)}%</span>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 11, color: T.text, marginBottom: 3 }}>{d.label}</div>
            <BarThin value={d.weight * 10} color={T.sageD} />
          </div>
        </div>
      ))}

      {/* Values + Working */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 20, marginTop: 4 }}>
        <div>
          <Rule label="values" />
          {PROFILE.values.map((v, i) => (
            <div key={i} style={{ fontSize: 10, color: T.muted, marginBottom: 6, paddingLeft: 10, borderLeft: `1px solid ${T.sageD}` }}>{v}</div>
          ))}
        </div>
        <div>
          <Rule label="working style" />
          {Object.entries(PROFILE.working).map(([k, v]) => (
            <div key={k} style={{ marginBottom: 8 }}>
              <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.1em", textTransform: "uppercase", marginBottom: 2 }}>{k}</div>
              <div style={{ fontSize: 10, color: T.muted }}>{v}</div>
            </div>
          ))}
        </div>
      </div>

      {/* Pending */}
      {PROFILE.deltas > 0 && (
        <>
          <Rule label="pending attention" />
          <div style={{ background: T.lift, border: `1px solid ${T.border}`, borderLeft: `2px solid ${T.gold}`, borderRadius: 4, padding: "10px 12px", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
            <span style={{ fontSize: 11, color: T.muted }}>{PROFILE.deltas} unresolved conflict{PROFILE.deltas !== 1 ? "s" : ""} · {SESSION_OBSERVATIONS.length} new observations this session</span>
            <Badge color={T.gold}>needs you</Badge>
          </div>
        </>
      )}
    </div>
  );
}

// ── CHECK-IN MODE ────────────────────────────────────────────────────────────
function CheckinPanel({ onBack }) {
  const [obs, setObs] = useState(SESSION_OBSERVATIONS.map(o => ({ ...o })));
  const [annotating, setAnnotating] = useState(null);
  const [annotText, setAnnotText] = useState("");
  const [deltaIdx, setDeltaIdx] = useState(0);
  const [deltaResolved, setDeltaResolved] = useState(false);

  const act = (id, action) => {
    setObs(prev => prev.map(o => o.id === id ? { ...o, status: action } : o));
    if (annotating === id) setAnnotating(null);
  };

  const pending = obs.filter(o => o.status === "pending");
  const held    = obs.filter(o => o.status === "hold");
  const skipped = obs.filter(o => o.status === "skip");

  const typeColor = (type) => ({
    signal: T.sage, ironic_hedging: T.gold, working: T.sageL, domain: T.muted,
  }[type] || T.muted);

  return (
    <div style={{ width: "100%", maxWidth: 620, fontFamily: "monospace" }}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 24 }}>
        <div>
          <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 18, color: T.cream, letterSpacing: "-0.02em" }}>
            check<span style={{ color: T.sage }}>·</span>in
          </div>
          <div style={{ fontSize: 9, color: T.muted, marginTop: 3 }}>
            {new Date().toLocaleDateString("en-US", { month: "long", day: "numeric", year: "numeric" })} · {SESSION_OBSERVATIONS.length} observations this session
          </div>
        </div>
        <button onClick={onBack} style={{ fontSize: 9, padding: "5px 10px", background: "transparent", border: `1px solid ${T.border}`, borderRadius: 4, color: T.muted, cursor: "pointer", fontFamily: "monospace", letterSpacing: "0.08em" }}>← BACK</button>
      </div>

      {/* Engine's session summary */}
      <div style={{ background: T.lift, border: `1px solid ${T.border}`, borderRadius: 6, padding: "14px 16px", marginBottom: 24 }}>
        <div style={{ fontSize: 9, color: T.sageD, letterSpacing: "0.12em", marginBottom: 8 }}>ENGINE · SESSION SUMMARY</div>
        <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 13, color: T.text, lineHeight: 1.7, fontStyle: "italic" }}>
          Four observations this session. Your spatial-metaphor tendency held — "topography" three times. One ironic hedge caught and rerouted. The co-creator pattern is stable. Schema design may be worth its own domain entry.
        </div>
      </div>

      {/* New observations */}
      {pending.length > 0 && (
        <>
          <Rule label={`this session · ${pending.length} pending`} />
          {pending.map(o => (
            <div key={o.id} style={{ background: T.surface, border: `1px solid ${T.border}`, borderRadius: 6, marginBottom: 10, overflow: "hidden" }}>
              <div style={{ padding: "12px 14px" }}>
                <div style={{ display: "flex", gap: 6, marginBottom: 8, alignItems: "center" }}>
                  <Badge color={typeColor(o.type)}>{o.type.replace("_", " ")}</Badge>
                  <span style={{ fontSize: 8, color: T.faint, fontFamily: "monospace" }}>{o.field}</span>
                </div>
                <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 12, color: T.text, lineHeight: 1.65, fontStyle: "italic", marginBottom: 10 }}>
                  {o.engine}
                </div>
                {annotating === o.id && (
                  <div style={{ marginBottom: 10 }}>
                    <textarea
                      value={annotText}
                      onChange={e => setAnnotText(e.target.value)}
                      placeholder="Add your annotation…"
                      style={{
                        width: "100%", background: T.bg, border: `1px solid ${T.borderHi}`, borderRadius: 4,
                        color: T.text, fontSize: 11, fontFamily: "Fraunces, Georgia, serif", fontStyle: "italic",
                        padding: "8px 10px", resize: "vertical", minHeight: 60, outline: "none", boxSizing: "border-box",
                      }}
                    />
                  </div>
                )}
                <div style={{ display: "flex", gap: 6 }}>
                  <button onClick={() => act(o.id, "hold")} style={{ flex: 1, padding: "6px 0", fontSize: 9, letterSpacing: "0.08em", background: T.sage + "18", border: `1px solid ${T.sage}40`, borderRadius: 3, color: T.sage, cursor: "pointer", fontFamily: "monospace" }}>HOLD</button>
                  <button onClick={() => setAnnotating(annotating === o.id ? null : o.id)} style={{ flex: 1, padding: "6px 0", fontSize: 9, letterSpacing: "0.08em", background: T.gold + "12", border: `1px solid ${T.gold}30`, borderRadius: 3, color: T.gold, cursor: "pointer", fontFamily: "monospace" }}>ADJUST</button>
                  <button onClick={() => act(o.id, "skip")} style={{ flex: 1, padding: "6px 0", fontSize: 9, letterSpacing: "0.08em", background: T.lift, border: `1px solid ${T.border}`, borderRadius: 3, color: T.muted, cursor: "pointer", fontFamily: "monospace" }}>SKIP</button>
                </div>
              </div>
            </div>
          ))}
        </>
      )}

      {/* Held this session */}
      {held.length > 0 && (
        <>
          <Rule label={`confirmed · ${held.length}`} />
          {held.map(o => (
            <div key={o.id} style={{ display: "flex", gap: 10, alignItems: "flex-start", marginBottom: 8, padding: "8px 10px", background: T.lift, borderRadius: 4, border: `1px solid ${T.border}` }}>
              <Dot on />
              <span style={{ fontSize: 11, color: T.muted, fontStyle: "italic", fontFamily: "Fraunces, Georgia, serif" }}>{o.value}</span>
              <Badge color={T.sage}>held</Badge>
            </div>
          ))}
        </>
      )}

      {/* What's holding — stable observations */}
      <Rule label="what's holding" />
      <div style={{ fontSize: 9, color: T.muted, marginBottom: 10, letterSpacing: "0.05em" }}>
        Observations confirmed across multiple sessions. These are stable.
      </div>
      {STABLE.map((s, i) => (
        <div key={i} style={{ display: "flex", gap: 12, alignItems: "flex-start", marginBottom: 10, padding: "10px 12px", background: T.surface, border: `1px solid ${T.border}`, borderRadius: 5 }}>
          <div style={{ flexShrink: 0, textAlign: "right", minWidth: 40 }}>
            <div style={{ fontSize: 9, color: T.sage }}>{(s.confidence * 100).toFixed(0)}%</div>
            <div style={{ fontSize: 8, color: T.faint, marginTop: 1 }}>{s.since}</div>
          </div>
          <span style={{ fontSize: 11, color: T.text, lineHeight: 1.55, fontFamily: "Fraunces, Georgia, serif", fontStyle: "italic" }}>{s.value}</span>
        </div>
      ))}

      {/* Delta */}
      {!deltaResolved && (
        <>
          <Rule label="needs you · conflicts" />
          {DELTAS.map((d, i) => (
            <div key={i} style={{ background: T.surface, border: `1px solid ${T.gold}30`, borderLeft: `2px solid ${T.gold}`, borderRadius: 6, padding: "12px 14px", marginBottom: 10 }}>
              <div style={{ fontSize: 9, color: T.faint, letterSpacing: "0.1em", marginBottom: 8 }}>{d.field}</div>
              <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 12, color: T.text, lineHeight: 1.65, fontStyle: "italic", marginBottom: 10 }}>{d.note}</div>
              <div style={{ display: "flex", gap: 8, marginBottom: 10 }}>
                <div style={{ flex: 1, background: T.lift, borderRadius: 4, padding: "8px 10px", border: `1px solid ${T.border}` }}>
                  <div style={{ fontSize: 8, color: T.muted, marginBottom: 3 }}>{d.a.orientation}</div>
                  <div style={{ fontSize: 10, color: T.text }}>{d.a.value}</div>
                  <div style={{ fontSize: 8, color: T.faint, marginTop: 2 }}>conf {(d.a.conf * 100).toFixed(0)}%</div>
                </div>
                <div style={{ flex: 1, background: T.lift, borderRadius: 4, padding: "8px 10px", border: `1px solid ${T.border}` }}>
                  <div style={{ fontSize: 8, color: T.muted, marginBottom: 3 }}>{d.b.orientation}</div>
                  <div style={{ fontSize: 10, color: T.text }}>{d.b.value}</div>
                  <div style={{ fontSize: 8, color: T.faint, marginTop: 2 }}>conf {(d.b.conf * 100).toFixed(0)}%</div>
                </div>
              </div>
              <div style={{ display: "flex", gap: 6 }}>
                <button onClick={() => setDeltaResolved(true)} style={{ flex: 1, padding: "6px 0", fontSize: 9, background: T.sage + "18", border: `1px solid ${T.sage}40`, borderRadius: 3, color: T.sage, cursor: "pointer", fontFamily: "monospace", letterSpacing: "0.08em" }}>CONFIRM BOTH</button>
                <button onClick={() => setDeltaResolved(true)} style={{ flex: 1, padding: "6px 0", fontSize: 9, background: T.lift, border: `1px solid ${T.border}`, borderRadius: 3, color: T.muted, cursor: "pointer", fontFamily: "monospace", letterSpacing: "0.08em" }}>DEFER</button>
              </div>
            </div>
          ))}
        </>
      )}
      {deltaResolved && (
        <div style={{ fontSize: 10, color: T.sageD, padding: "8px 0", fontFamily: "Fraunces, Georgia, serif", fontStyle: "italic" }}>
          Conflict resolved. Profile updated.
        </div>
      )}

      {/* Skipped */}
      {skipped.length > 0 && (
        <div style={{ marginTop: 16, fontSize: 9, color: T.faint }}>{skipped.length} observation{skipped.length !== 1 ? "s" : ""} skipped — still proposed, revisit anytime.</div>
      )}
    </div>
  );
}

// ── ROOT ─────────────────────────────────────────────────────────────────────
export default function PersonalityIndexer() {
  const [mode, setMode] = useState("compact");

  useEffect(() => {
    const style = document.createElement("style");
    style.textContent = `
      @import url('https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,300;0,9..144,400;1,9..144,300;1,9..144,400&family=JetBrains+Mono:wght@300;400&display=swap');
      @keyframes breathe { 0%,100%{opacity:1;} 50%{opacity:0.35;} }
      @keyframes rise { from{opacity:0;transform:translateY(6px);} to{opacity:1;transform:translateY(0);} }
      * { box-sizing: border-box; }
      button { transition: opacity 0.15s; } button:hover { opacity: 0.8; }
      textarea { transition: border-color 0.2s; } textarea:focus { border-color: #7fa88a !important; }
    `;
    document.head.appendChild(style);
    return () => document.head.removeChild(style);
  }, []);

  return (
    <div style={{
      minHeight: "100vh", background: T.bg, display: "flex",
      alignItems: mode === "compact" ? "center" : "flex-start",
      justifyContent: "center",
      padding: mode === "compact" ? 40 : "36px 24px",
      animation: "rise 0.4s ease",
    }}>
      {mode === "compact"  && <CompactPanel onExpand={() => setMode("visual")} onCheckin={() => setMode("checkin")} />}
      {mode === "visual"   && <VisualPanel  onCheckin={() => setMode("checkin")} onCompact={() => setMode("compact")} />}
      {mode === "checkin"  && <CheckinPanel onBack={() => setMode("visual")} />}
    </div>
  );
}
