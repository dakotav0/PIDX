import { useState, useEffect, useRef } from "react";

const T = {
  bg: "#0e0f0d", surface: "#161714", lift: "#1d1f1b",
  border: "#252720", borderHi: "#3a3d35",
  text: "#d4cfc7", muted: "#7a786f", faint: "#3a3830",
  sage: "#7fa88a", sageD: "#4a6b56", sageL: "#a8c9b4",
  gold: "#c9a96e", goldD: "#8a7040",
  cream: "#ede8df", red: "#c47a6a",
};

const USER_A = {
  id: "usr_default", label: "dakota",
  register: { formality: 2.1, directness: 7.8, hedging: 2.9, humor: 6.3, abstraction: 7.1 },
  domains: ["schema design", "skill engineering", "NLP/personality", "Svelte"],
  values: ["consolidation over accumulation", "architecture as values act", "data sovereignty"],
  working: { mode: "sketch-level, extrapolates", feedback: "two-word ratification", pace: "burst-capable" },
  confidence: 0.88,
};

const USER_B = {
  id: "usr_alice", label: "alice",
  register: { formality: 6.4, directness: 4.2, hedging: 5.8, humor: 3.1, abstraction: 4.5 },
  domains: ["product management", "user research", "data analysis", "Figma"],
  values: ["user empathy first", "iterative shipping", "cross-functional clarity"],
  working: { mode: "detailed spec, minimal inference", feedback: "structured critique", pace: "steady cadence" },
  confidence: 0.74,
};

const AXES = ["formality", "directness", "hedging", "humor", "abstraction"];
const AXIS_LABELS = ["Formal", "Direct", "Hedging", "Humor", "Abstract"];

function RadarChart({ a, b, width = 300, height = 300 }) {
  const cx = width / 2, cy = height / 2;
  const r = Math.min(width, height) / 2 - 40;
  const n = AXES.length;

  const angle = (i) => (Math.PI * 2 * i) / n - Math.PI / 2;
  const pt = (val, i) => {
    const a = angle(i), rv = (val / 10) * r;
    return [cx + rv * Math.cos(a), cy + rv * Math.sin(a)];
  };
  const labelPt = (i) => {
    const a = angle(i), rv = r + 22;
    return [cx + rv * Math.cos(a), cy + rv * Math.sin(a)];
  };

  const polyA = AXES.map((k, i) => pt(a.register[k], i));
  const polyB = AXES.map((k, i) => pt(b.register[k], i));

  const gridRings = [2, 4, 6, 8, 10];

  // Tension lines — axes where diff > 2.5
  const tensionPairs = [];
  for (let i = 0; i < n; i++) {
    for (let j = i + 2; j < n; j++) {
      const diffI = Math.abs(a.register[AXES[i]] - b.register[AXES[i]]);
      const diffJ = Math.abs(a.register[AXES[j]] - b.register[AXES[j]]);
      if (diffI > 2.5 && diffJ > 2.5) tensionPairs.push([i, j]);
    }
  }

  return (
    <svg width={width} height={height} style={{ overflow: "visible" }}>
      {/* Grid rings */}
      {gridRings.map((rv) => {
        const pts = AXES.map((_, i) => {
          const [x, y] = [cx + (rv / 10) * r * Math.cos(angle(i)), cy + (rv / 10) * r * Math.sin(angle(i))];
          return `${x},${y}`;
        }).join(" ");
        return <polygon key={rv} points={pts} fill="none" stroke={T.border} strokeWidth={0.5} />;
      })}

      {/* Axis spokes */}
      {AXES.map((_, i) => {
        const [x, y] = [cx + r * Math.cos(angle(i)), cy + r * Math.sin(angle(i))];
        return <line key={i} x1={cx} y1={cy} x2={x} y2={y} stroke={T.border} strokeWidth={0.5} />;
      })}

      {/* Tension lines (enneagram internal connections) */}
      {tensionPairs.map(([i, j], idx) => {
        const [ax, ay] = pt((a.register[AXES[i]] + b.register[AXES[i]]) / 2, i);
        const [bx, by] = pt((a.register[AXES[j]] + b.register[AXES[j]]) / 2, j);
        return (
          <line key={idx} x1={ax} y1={ay} x2={bx} y2={by}
            stroke={T.gold} strokeWidth={0.8} strokeDasharray="3 3" opacity={0.4} />
        );
      })}

      {/* User B polygon */}
      <polygon
        points={polyB.map(([x, y]) => `${x},${y}`).join(" ")}
        fill={T.gold + "22"} stroke={T.gold} strokeWidth={1.2} strokeDasharray="4 3"
      />

      {/* User A polygon */}
      <polygon
        points={polyA.map(([x, y]) => `${x},${y}`).join(" ")}
        fill={T.sage + "28"} stroke={T.sage} strokeWidth={1.5}
      />

      {/* Data points A */}
      {polyA.map(([x, y], i) => (
        <circle key={i} cx={x} cy={y} r={3} fill={T.sage} />
      ))}
      {/* Data points B */}
      {polyB.map(([x, y], i) => (
        <circle key={i} cx={x} cy={y} r={3} fill={T.gold} />
      ))}

      {/* Axis labels */}
      {AXES.map((k, i) => {
        const [x, y] = labelPt(i);
        const diffBig = Math.abs(a.register[k] - b.register[k]) > 2.5;
        return (
          <text key={i} x={x} y={y}
            textAnchor="middle" dominantBaseline="middle"
            fontSize={9} fontFamily="monospace" letterSpacing="0.08em"
            fill={diffBig ? T.gold : T.muted}
          >
            {AXIS_LABELS[i].toUpperCase()}
          </text>
        );
      })}
    </svg>
  );
}

function BarDiff({ label, a, b }) {
  const diff = a - b;
  const absDiff = Math.abs(diff);
  const significant = absDiff > 2.5;
  return (
    <div style={{ marginBottom: 10 }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 4 }}>
        <span style={{ fontSize: 9, color: significant ? T.gold : T.muted, letterSpacing: "0.1em", textTransform: "uppercase" }}>{label}</span>
        <span style={{ fontSize: 9, color: significant ? T.gold : T.faint }}>
          {diff > 0 ? "+" : ""}{diff.toFixed(1)}
        </span>
      </div>
      <div style={{ display: "flex", gap: 3, alignItems: "center", height: 6 }}>
        <div style={{ flex: 1, background: T.faint, borderRadius: 1, height: 3, overflow: "hidden", position: "relative" }}>
          <div style={{ position: "absolute", right: 0, width: `${(a / 10) * 100}%`, height: "100%", background: T.sage, borderRadius: 1 }} />
        </div>
        <div style={{ width: 1, height: 10, background: T.borderHi, flexShrink: 0 }} />
        <div style={{ flex: 1, background: T.faint, borderRadius: 1, height: 3, overflow: "hidden" }}>
          <div style={{ width: `${(b / 10) * 100}%`, height: "100%", background: T.gold, borderRadius: 1 }} />
        </div>
      </div>
    </div>
  );
}

function ConfidenceArc({ value, size = 60, color = T.sage }) {
  const r = (size / 2) - 6;
  const circ = 2 * Math.PI * r;
  const dash = (value * circ).toFixed(1);
  return (
    <svg width={size} height={size}>
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={T.faint} strokeWidth={2} />
      <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke={color} strokeWidth={2}
        strokeDasharray={`${dash} ${circ}`}
        strokeLinecap="round"
        style={{ transform: `rotate(-90deg)`, transformOrigin: "50% 50%" }}
      />
      <text x={size / 2} y={size / 2} textAnchor="middle" dominantBaseline="middle"
        fontSize={10} fill={color} fontFamily="monospace">
        {Math.round(value * 100)}%
      </text>
    </svg>
  );
}

const VISUALIZER_IDEAS = [
  {
    cmd: "diff",
    name: "Register matrix",
    desc: "Radar polygon with internal tension lines between high-divergence axes. The enneagram mechanic — internal connections reveal relational tensions, not just isolated scores.",
    status: "built",
  },
  {
    cmd: "status",
    name: "Observation cascade",
    desc: "Waterfall by field — each field is a horizontal lane. Bars show proposed / confirmed / delta counts. Clicking a lane calls `pidx status <user> --format json` and drills in.",
    status: "next",
  },
  {
    cmd: "show",
    name: "Decay horizon",
    desc: "Timeline per field class showing effective confidence over time. Each λ class is a curve family. Decay-exempt observations appear as flat lines above the threshold.",
    status: "next",
  },
  {
    cmd: "bridge",
    name: "Orientation provenance",
    desc: "Stacked origin chart — for each confirmed observation, a small pill showing which orientations contributed and their base confidence. Shows where profile weight actually comes from.",
    status: "next",
  },
  {
    cmd: "diff",
    name: "Domain overlap Venn",
    desc: "Two overlapping bubbles — shared domains in the intersection, unique domains in each wing. Bubble size encodes weight. Useful social artifact — shareable diff rendering.",
    status: "ideas",
  },
  {
    cmd: "review",
    name: "Decay heat map",
    desc: "Grid of all observations × time. Cell color encodes effective confidence. Review queue items pulse. Clicking runs `pidx review process`.",
    status: "ideas",
  },
];

const STATUS_COLOR = { built: T.sage, next: T.gold, ideas: T.muted };
const STATUS_LABEL = { built: "built", next: "next", ideas: "idea" };

export default function PidxDiff() {
  const [tab, setTab] = useState("diff");

  useEffect(() => {
    const s = document.createElement("style");
    s.textContent = `
      @import url('https://fonts.googleapis.com/css2?family=Fraunces:ital,opsz,wght@0,9..144,300;0,9..144,400;1,9..144,300;1,9..144,400&family=JetBrains+Mono:wght@300;400&display=swap');
      @keyframes rise { from{opacity:0;transform:translateY(5px)}to{opacity:1;transform:translateY(0)} }
      * { box-sizing:border-box; }
      button:hover { opacity:0.75; }
    `;
    document.head.appendChild(s);
    return () => document.head.removeChild(s);
  }, []);

  const bigDiffs = AXES.filter(k => Math.abs(USER_A.register[k] - USER_B.register[k]) > 2.5);

  return (
    <div style={{ minHeight: "100vh", background: T.bg, color: T.text, fontFamily: "JetBrains Mono, monospace", padding: "28px 24px 60px" }}>

      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 24, flexWrap: "wrap", gap: 12 }}>
        <div>
          <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 20, color: T.cream, letterSpacing: "-0.02em" }}>
            pidx<span style={{ color: T.sage }}>·</span>diff
          </div>
          <div style={{ fontSize: 9, color: T.muted, marginTop: 3, letterSpacing: "0.1em" }}>
            visualizer suite · {bigDiffs.length} axis tension{bigDiffs.length !== 1 ? "s" : ""} detected
          </div>
        </div>
        <div style={{ display: "flex", gap: 6 }}>
          {["diff", "suite"].map(t => (
            <button key={t} onClick={() => setTab(t)} style={{
              fontSize: 9, padding: "5px 12px", letterSpacing: "0.1em", textTransform: "uppercase",
              background: tab === t ? T.sage + "18" : T.lift,
              border: `1px solid ${tab === t ? T.sage + "50" : T.border}`,
              color: tab === t ? T.sage : T.muted,
              borderRadius: 4, cursor: "pointer", fontFamily: "monospace",
            }}>{t}</button>
          ))}
        </div>
      </div>

      {tab === "diff" && (
        <div style={{ animation: "rise 0.3s ease" }}>

          {/* User identity cards */}
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10, marginBottom: 20 }}>
            {[USER_A, USER_B].map((u, i) => (
              <div key={i} style={{ background: T.surface, border: `1px solid ${i === 0 ? T.sageD : T.goldD}`, borderRadius: 6, padding: "12px 14px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 6 }}>
                  <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 14, color: T.cream, fontStyle: "italic" }}>{u.label}</div>
                  <ConfidenceArc value={u.confidence} size={44} color={i === 0 ? T.sage : T.gold} />
                </div>
                <div style={{ fontSize: 9, color: T.faint, letterSpacing: "0.08em" }}>{u.id}</div>
                <div style={{ marginTop: 8, display: "flex", flexWrap: "wrap", gap: 3 }}>
                  {u.domains.slice(0, 3).map((d, j) => (
                    <span key={j} style={{ fontSize: 8, padding: "1px 5px", borderRadius: 2, background: (i === 0 ? T.sage : T.gold) + "14", color: i === 0 ? T.sageL : T.gold, border: `1px solid ${(i === 0 ? T.sage : T.gold)}30`, letterSpacing: "0.06em" }}>{d}</span>
                  ))}
                </div>
              </div>
            ))}
          </div>

          {/* Main diff layout */}
          <div style={{ display: "grid", gridTemplateColumns: "300px 1fr", gap: 20, alignItems: "start" }}>

            {/* Radar */}
            <div style={{ background: T.surface, border: `1px solid ${T.border}`, borderRadius: 8, padding: 16, display: "flex", flexDirection: "column", alignItems: "center" }}>
              <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.12em", marginBottom: 12, textTransform: "uppercase" }}>register topology</div>
              <RadarChart a={USER_A} b={USER_B} width={240} height={240} />
              <div style={{ display: "flex", gap: 14, marginTop: 12 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
                  <div style={{ width: 12, height: 2, background: T.sage, borderRadius: 1 }} />
                  <span style={{ fontSize: 9, color: T.muted }}>{USER_A.label}</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
                  <div style={{ width: 12, height: 2, background: T.gold, borderRadius: 1, borderTop: `1px dashed ${T.gold}`, borderBottom: "none" }} />
                  <span style={{ fontSize: 9, color: T.muted }}>{USER_B.label}</span>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 5 }}>
                  <div style={{ width: 12, height: 1, background: T.gold, opacity: 0.4, borderTop: `1px dashed ${T.gold}` }} />
                  <span style={{ fontSize: 9, color: T.faint }}>tension</span>
                </div>
              </div>
              {bigDiffs.length > 0 && (
                <div style={{ marginTop: 12, padding: "8px 10px", background: T.lift, borderRadius: 4, border: `1px solid ${T.border}`, width: "100%" }}>
                  <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.1em", marginBottom: 4 }}>HIGH DIVERGENCE</div>
                  {bigDiffs.map(k => (
                    <div key={k} style={{ fontSize: 9, color: T.gold, marginBottom: 2 }}>
                      {k} · Δ{Math.abs(USER_A.register[k] - USER_B.register[k]).toFixed(1)}
                    </div>
                  ))}
                </div>
              )}
            </div>

            {/* Right panel */}
            <div>
              {/* Bar diffs */}
              <div style={{ background: T.surface, border: `1px solid ${T.border}`, borderRadius: 8, padding: "14px 16px", marginBottom: 12 }}>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 12 }}>
                  <span style={{ fontSize: 8, color: T.faint, letterSpacing: "0.12em" }}>REGISTER DIFF</span>
                  <div style={{ display: "flex", gap: 10 }}>
                    <span style={{ fontSize: 8, color: T.sage }}>{USER_A.label}</span>
                    <span style={{ fontSize: 8, color: T.gold }}>{USER_B.label}</span>
                  </div>
                </div>
                {AXES.map(k => (
                  <BarDiff key={k} label={k} a={USER_A.register[k]} b={USER_B.register[k]} />
                ))}
              </div>

              {/* Working style diff */}
              <div style={{ background: T.surface, border: `1px solid ${T.border}`, borderRadius: 8, padding: "14px 16px", marginBottom: 12 }}>
                <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.12em", marginBottom: 12 }}>WORKING STYLE</div>
                {Object.keys(USER_A.working).map(k => {
                  const same = USER_A.working[k] === USER_B.working[k];
                  return (
                    <div key={k} style={{ marginBottom: 10 }}>
                      <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.1em", textTransform: "uppercase", marginBottom: 4 }}>{k}</div>
                      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>
                        <div style={{ fontSize: 10, color: T.text, padding: "5px 8px", background: T.lift, borderRadius: 3, borderLeft: `2px solid ${T.sage}` }}>{USER_A.working[k]}</div>
                        <div style={{ fontSize: 10, color: T.muted, padding: "5px 8px", background: T.lift, borderRadius: 3, borderLeft: `2px solid ${T.gold}` }}>{USER_B.working[k]}</div>
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Values overlap */}
              <div style={{ background: T.surface, border: `1px solid ${T.border}`, borderRadius: 8, padding: "14px 16px" }}>
                <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.12em", marginBottom: 10 }}>VALUES</div>
                <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 6 }}>
                  <div>
                    {USER_A.values.map((v, i) => (
                      <div key={i} style={{ fontSize: 9, color: T.sageL, padding: "3px 6px", marginBottom: 3, background: T.sage + "10", borderRadius: 2 }}>{v}</div>
                    ))}
                  </div>
                  <div>
                    {USER_B.values.map((v, i) => (
                      <div key={i} style={{ fontSize: 9, color: T.gold, padding: "3px 6px", marginBottom: 3, background: T.gold + "10", borderRadius: 2 }}>{v}</div>
                    ))}
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {tab === "suite" && (
        <div style={{ animation: "rise 0.3s ease" }}>
          <div style={{ fontSize: 9, color: T.muted, marginBottom: 20, letterSpacing: "0.05em" }}>
            Visualizers mapped to CLI commands. Each consumes `--format json` output from the Rust binary.
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10 }}>
            {VISUALIZER_IDEAS.map((v, i) => (
              <div key={i} style={{
                background: T.surface, border: `1px solid ${T.border}`,
                borderRadius: 8, padding: "14px 16px",
                borderLeft: `2px solid ${STATUS_COLOR[v.status]}`,
                opacity: v.status === "ideas" ? 0.7 : 1,
              }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 8 }}>
                  <div>
                    <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 13, color: T.cream, fontStyle: "italic", marginBottom: 2 }}>{v.name}</div>
                    <div style={{ fontSize: 8, color: T.faint, fontFamily: "monospace", letterSpacing: "0.1em" }}>pidx {v.cmd} --format json</div>
                  </div>
                  <span style={{
                    fontSize: 8, padding: "2px 6px", borderRadius: 2, letterSpacing: "0.1em",
                    background: STATUS_COLOR[v.status] + "18",
                    color: STATUS_COLOR[v.status],
                    border: `1px solid ${STATUS_COLOR[v.status]}30`,
                  }}>{STATUS_LABEL[v.status]}</span>
                </div>
                <div style={{ fontSize: 10, color: T.muted, lineHeight: 1.6 }}>{v.desc}</div>
              </div>
            ))}
          </div>

          <div style={{ marginTop: 24, padding: "14px 16px", background: T.surface, border: `1px solid ${T.border}`, borderRadius: 8 }}>
            <div style={{ fontSize: 8, color: T.faint, letterSpacing: "0.12em", marginBottom: 10 }}>MCP SURFACE</div>
            <div style={{ fontFamily: "Fraunces, Georgia, serif", fontSize: 12, color: T.muted, lineHeight: 1.7, fontStyle: "italic" }}>
              Each CLI command becomes an MCP tool directly — the Rust lib already has the right interface shape. <code style={{ fontFamily: "JetBrains Mono", fontSize: 10, background: T.lift, padding: "1px 4px", borderRadius: 2 }}>pidx_show</code>, <code style={{ fontFamily: "JetBrains Mono", fontSize: 10, background: T.lift, padding: "1px 4px", borderRadius: 2 }}>pidx_ingest</code>, <code style={{ fontFamily: "JetBrains Mono", fontSize: 10, background: T.lift, padding: "1px 4px", borderRadius: 2 }}>pidx_confirm</code>, <code style={{ fontFamily: "JetBrains Mono", fontSize: 10, background: T.lift, padding: "1px 4px", borderRadius: 2 }}>pidx_diff</code>. The lib+bin split means the MCP server is just another binary target consuming the same lib. No new logic — just transport.
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
