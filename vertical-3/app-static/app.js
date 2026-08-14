const $ = (id) => document.getElementById(id);

/** Single product tenant for pilot sales path (not lab ten_demo). */
const PILOT_TENANT = "ten_github";

/**
 * Display timezone: India Standard Time (IST = UTC+05:30, no DST).
 * Never invents times — only converts real Date/ISO instants with exact offset.
 */
const IST_TZ = "Asia/Kolkata";
const IST_LABEL = "IST";

/** Parse ISO/RFC3339 (or Date) → Date; invalid → null (do not invent). */
function parseInstant(input) {
  if (input == null || input === "") return null;
  if (input instanceof Date) {
    return Number.isNaN(input.getTime()) ? null : input;
  }
  const s = String(input).trim();
  if (!s) return null;
  // Native Date parses Z / offsets correctly.
  const d = new Date(s);
  if (Number.isNaN(d.getTime())) return null;
  return d;
}

/** Format instant as listed IST wall time: `2026-08-13 17:30 IST`. */
function fmtIst(input, opts) {
  const d = parseInstant(input);
  if (!d) return input == null || input === "" ? "—" : String(input);
  try {
    const parts = new Intl.DateTimeFormat("en-GB", {
      timeZone: IST_TZ,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }).formatToParts(d);
    const get = (t) => parts.find((p) => p.type === t)?.value || "";
    const y = get("year");
    const mo = get("month");
    const day = get("day");
    const h = get("hour");
    const mi = get("minute");
    if (opts?.compact) return `${mo}-${day} ${h}:${mi} ${IST_LABEL}`;
    if (opts?.dateOnly) return `${y}-${mo}-${day} ${IST_LABEL}`;
    if (opts?.withSeconds) {
      const secParts = new Intl.DateTimeFormat("en-GB", {
        timeZone: IST_TZ,
        second: "2-digit",
        hour12: false,
      }).formatToParts(d);
      const sec = secParts.find((p) => p.type === "second")?.value || "00";
      return `${y}-${mo}-${day} ${h}:${mi}:${sec} ${IST_LABEL}`;
    }
    return `${y}-${mo}-${day} ${h}:${mi} ${IST_LABEL}`;
  } catch (_) {
    return String(input);
  }
}

/** Now listed in IST (from real clock). */
function nowIstList() {
  return fmtIst(new Date());
}

let state = {
  tenant: PILOT_TENANT,
  draftId: null,
  ledgerId: null,
  latest: null,
  status: null,
};

/**
 * Simple vs Technical = presentation only (not different feature sets).
 * Simple: plain English, visual cards, human names — same data underneath.
 * Technical: machine tags, IDs, raw status codes, JSON-ish stats.
 */
const UX_MODE_KEY = "ai_manager_ux_mode";
function getUxMode() {
  try {
    const m = localStorage.getItem(UX_MODE_KEY);
    if (m === "technical" || m === "simple") return m;
  } catch (_) {}
  return "simple";
}
function isSimpleMode() {
  return getUxMode() === "simple";
}
function setUxMode(mode, opts) {
  const m = mode === "technical" ? "technical" : "simple";
  try {
    localStorage.setItem(UX_MODE_KEY, m);
  } catch (_) {}
  document.body.classList.remove("mode-simple", "mode-technical");
  document.body.classList.add(m === "technical" ? "mode-technical" : "mode-simple");
  // Page names stay the same in both modes — only update the label span (keep icons).
  document.querySelectorAll(".nav-item[data-label-simple]").forEach((btn) => {
    const s = btn.getAttribute("data-label-simple");
    const t = btn.getAttribute("data-label-tech");
    const label = btn.querySelector(".nav-label");
    const text = s || t;
    if (label && text) label.textContent = text;
  });
  document.querySelectorAll(".ux-mode-btn").forEach((btn) => {
    btn.textContent = m === "technical" ? "Simple view" : "Technical view";
    btn.setAttribute("aria-pressed", m === "simple" ? "true" : "false");
    btn.title =
      m === "technical"
        ? "Simple view: visual story, plain English"
        : "Technical view: same product, denser detail (ids behind the eye)";
  });
  if ($("settings-ux-mode")) {
    $("settings-ux-mode").textContent =
      m === "simple"
        ? "Mode: Simple — visual story, plain English. Same data."
        : "Mode: Technical — same data with tags and denser detail. Identifiers sit behind the eye.";
  }
  if ($("nav-mode")) $("nav-mode").textContent = m === "simple" ? "Simple view" : "Technical view";
  const navSub = document.querySelector(".nav-sub");
  if (navSub) navSub.textContent = "What needs attention";
  if (opts?.rerender !== false) {
    try {
      rerenderActiveViewForUx();
    } catch (_) {}
  }
}
function toggleUxMode(ev) {
  if (ev) {
    ev.preventDefault();
    ev.stopPropagation();
  }
  setUxMode(getUxMode() === "simple" ? "technical" : "simple");
}
function rerenderActiveViewForUx() {
  const active = document.querySelector(".nav-item.active");
  const v = active?.getAttribute("data-view") || "cockpit";
  if (typeof applyChromeUxMode === "function") applyChromeUxMode();
  if (v === "cockpit" && typeof refreshCockpit === "function") refreshCockpit();
  if (v === "team" && typeof refreshTeam === "function") refreshTeam();
  if (v === "graph" && typeof refreshGraph === "function") refreshGraph(false);
  if (v === "connections") {
    if (typeof refreshConnectors === "function") refreshConnectors();
    if (typeof refreshHealth === "function") refreshHealth();
    if (typeof refreshOnboarding === "function") refreshOnboarding();
  }
  if (v === "insights" && typeof loadDevInsightsView === "function") loadDevInsightsView();
  if (v === "today" && typeof refreshToday === "function") refreshToday();
  if (v === "status" && typeof applyStatusUxMode === "function") applyStatusUxMode();
  if (typeof loadPlainInsights === "function") loadPlainInsights().catch(() => {});
  if (typeof loadCommitments === "function") loadCommitments().catch(() => {});
  if (typeof loadIntentLedger === "function") loadIntentLedger().catch(() => {});
}

/** Map machine intent/conflict tags → plain English (Simple view). */
function plainIntentType(ty) {
  const t = (ty || "").toString().toUpperCase();
  const map = {
    SHIP: "Trying to ship",
    BLOCKED: "Stuck / waiting",
    FREEZE: "Hold — don't merge yet",
    FIX: "Fixing something",
    EXPLORE: "Exploring",
    REVIEW: "In review",
    OTHER: "Other work",
    INTENT: "Work focus",
  };
  return map[t] || (ty ? String(ty) : "Work");
}
function plainConflictKind(kind) {
  const k = (kind || "").toString().toLowerCase();
  if (k.includes("ship") && k.includes("freeze")) return "Mixed signals (ship vs hold)";
  if (k.includes("dual") && k.includes("owner")) return "Unclear ownership";
  if (k.includes("block")) return "Blocker needs attention";
  if (k.includes("merge") || k.includes("friction")) return "Merge friction";
  if (k.includes("stale")) return "Review gone quiet";
  if (k.includes("ci")) return "Checks failing on ready work";
  return kind ? String(kind).replace(/_/g, " ") : "Team friction";
}
function plainSeverity(sev) {
  const s = (sev || "").toString().toLowerCase();
  if (s === "high") return "Urgent";
  if (s === "medium") return "Soon";
  if (s === "low") return "Watch";
  return sev || "";
}
function plainDigestStatus(d) {
  if (!d) return "No status update yet";
  if (d.empty_placeholder) return "Quiet window — nothing to report";
  const st = (d.status_label || d.status || "").toString().toLowerCase();
  if (st.includes("publish") || st === "shared") return "Shared with the team";
  if (st.includes("force") || st.includes("human") || st.includes("pending"))
    return "Needs your Approve / Don't send";
  if (st.includes("veto") || st.includes("dont")) return "Held back (don't send)";
  if (d.has_content) return "Update ready";
  return d.status_label || d.status || "Update";
}
function shortHumanId(id) {
  if (!id) return "";
  const s = String(id);
  if (s.length <= 14) return s;
  return s.slice(0, 8) + "…";
}

/** Opaque machine ids/hashes — never printed as first-view text. */
const OPAQUE_TOKEN_RE =
  /(?:[A-Za-z][\w-]*:(?:[^\s,;<>"'()[\]{}]{6,}))|(?:(?:gu|dft|led|cmt|cfl|evt)_[0-9A-Fa-f-]{8,})|(?:[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12})|(?:\b[0-9A-Fa-f]{7,40}\b)|(?:\b[UCDW][A-Z0-9]{8,}\b)/g;

function isOpaqueToken(s) {
  const t = String(s || "").trim();
  if (!t || t.length < 7) return false;
  if (/^\d+$/.test(t)) return false;
  if (/^ten_[a-z0-9_]+$/i.test(t)) return false;
  if (/^(SHIP|BLOCKED|FIX|EXPLORE|REVIEW|FREEZE|OTHER|HIGH|MEDIUM|LOW)$/i.test(t)) {
    return false;
  }
  if (/^(gu_|dft_|led_|cmt:|cfl_|twin:|intent:|commit:|event:|edge:|explicit:|person:|pr:|slack:)/i.test(t)) {
    return true;
  }
  if (/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(t)) {
    return true;
  }
  if (/^[0-9a-f]{7,40}$/i.test(t) && /[a-f]/i.test(t)) return true;
  if (/^[UCDW][A-Z0-9]{8,}$/.test(t)) return true;
  if (/^[a-z]{2,10}_[0-9a-f-]{8,}$/i.test(t)) return true;
  return false;
}

function idKindLabel(raw) {
  const s = String(raw || "");
  const pr = s.match(/\/pr\/(\d+)/i);
  if (pr || /^pr:/i.test(s)) return pr ? `PR #${pr[1]}` : "Pull request";
  if (/^commit:/i.test(s) || (/^[0-9a-f]{7,40}$/i.test(s) && /[a-f]/i.test(s))) {
    return "Commit";
  }
  if (/^intent:/i.test(s)) return "Intent";
  if (/^person:|^gu_/i.test(s)) return "Person";
  if (/^dft_/i.test(s)) return "Draft";
  if (/^led_/i.test(s)) return "Ledger";
  if (/^cmt:/i.test(s)) return "Promise";
  if (/^cfl_/i.test(s)) return "Conflict";
  if (/^twin:/i.test(s)) return "Record";
  if (/^explicit:/i.test(s)) return "Claim";
  if (/^event:|^edge:/i.test(s)) return "Evidence";
  if (/^U[A-Z0-9]{8,}$/.test(s)) return "Slack user";
  if (/^C[A-Z0-9]{8,}$/.test(s)) return "Channel";
  if (/^D[A-Z0-9]{8,}$/.test(s)) return "DM";
  if (/^W[A-Z0-9]{8,}$/.test(s)) return "Workspace";
  return "ID";
}

function idEye(raw) {
  const v = String(raw ?? "").trim();
  if (!v) return "";
  return (
    `<button type="button" class="id-eye" data-id="${esc(v)}" title="Reveal identifier" aria-label="Reveal ${esc(idKindLabel(v))}">` +
    `<svg viewBox="0 0 20 20" width="14" height="14" aria-hidden="true">` +
    `<circle cx="10" cy="10" r="8.4" fill="none" stroke="currentColor" stroke-width="1.5"/>` +
    `<ellipse cx="10" cy="10" rx="5" ry="3.15" fill="none" stroke="currentColor" stroke-width="1.45"/>` +
    `<circle cx="10" cy="10" r="1.35" fill="currentColor"/>` +
    `</svg>` +
    `<span class="id-eye-tip" role="tooltip">${esc(v)}</span>` +
    `</button>`
  );
}

/** Readable words + hoverable eye. Never dumps the raw token inline. */
function prettyRef(raw) {
  const v = String(raw ?? "").trim();
  if (!v) return "";
  if (!isOpaqueToken(v)) return esc(v);
  return `<span class="id-peek">${esc(idKindLabel(v))}${idEye(v)}</span>`;
}

function prettyMaybe(raw) {
  const v = String(raw ?? "").trim();
  if (!v) return "";
  const timeish = v.replace(/^at:/i, "");
  if (/^\d{4}-\d{2}-\d{2}T/.test(timeish)) return esc(fmtIst(timeish));
  return isOpaqueToken(v) ? prettyRef(v) : esc(v);
}

/** Walk prose and tuck opaque tokens behind eyes. Input is plain text. */
function scrubTextHtml(s) {
  const raw = scrubListedTimes(s ?? "");
  if (!raw) return "";
  const re = new RegExp(OPAQUE_TOKEN_RE.source, "g");
  let out = "";
  let last = 0;
  let m;
  while ((m = re.exec(raw))) {
    const tok = m[0];
    out += esc(raw.slice(last, m.index));
    out += isOpaqueToken(tok) ? prettyRef(tok) : esc(tok);
    last = m.index + tok.length;
  }
  out += esc(raw.slice(last));
  return out;
}

function displayNameOrEye(name, fallbackId) {
  const n = String(name || "").trim();
  if (n && !isOpaqueToken(n)) return esc(n);
  if (fallbackId) return prettyRef(fallbackId);
  return n ? prettyRef(n) : "—";
}

/** Champion-facing owner — never dump person:gu_* inline. */
function humanOwnerHtml(raw, members) {
  const t = String(raw || "").trim();
  if (!t) return "Someone";
  const stripped = t.replace(/^(twin:person:|twin:|person:)/i, "");
  const list = members || window.__ckMembers || [];
  const hit = list.find((p) => {
    const sid = String(p.subject_id || "");
    const name = String(p.display_name || "");
    return (
      sid === t ||
      sid === stripped ||
      name === t ||
      name === stripped ||
      p.person_node_id === t ||
      `person:${sid}` === t
    );
  });
  if (hit?.display_name && !isOpaqueToken(hit.display_name)) {
    return esc(hit.display_name);
  }
  if (isOpaqueToken(t) || /^(person:|twin:|gu_)/i.test(t) || t.includes(":gu_")) {
    return prettyRef(t);
  }
  return esc(t);
}

function amRow({ title, meta, tone, actionsHtml }) {
  const toneClass =
    tone === "urgent"
      ? "am-row--urgent"
      : tone === "soon"
        ? "am-row--soon"
        : tone === "ok"
          ? "am-row--ok"
          : "am-row--open";
  return `<div class="am-row ${toneClass}">
    <div class="am-row-body">
      <div class="am-row-title">${title}</div>
      ${meta ? `<div class="am-row-meta">${meta}</div>` : ""}
    </div>
    ${actionsHtml ? `<div class="am-row-actions">${actionsHtml}</div>` : ""}
  </div>`;
}

function heatBarsHtml(counts, labels) {
  const arr = counts || [];
  const labs = labels || [];
  const max = Math.max(...arr, 1);
  let html = `<div class="ux-heat-bars">`;
  let any = false;
  for (let i = 0; i < arr.length; i++) {
    const n = arr[i] || 0;
    if (!n) continue;
    any = true;
    const pct = Math.min(100, Math.round((n / max) * 100));
    html += `<div class="ux-heat-row"><span class="ux-heat-lab">${esc(labs[i] || String(i))}</span>
      <span class="ux-heat-bar" style="width:${pct}%"></span>
      <span class="muted small">${n}</span></div>`;
  }
  html += `</div>`;
  return any ? html : `<span class="muted">No activity yet.</span>`;
}

function activeTenant() {
  return (
    $("team-tenant")?.value?.trim() ||
    $("graph-tenant")?.value?.trim() ||
    $("tenant")?.value?.trim() ||
    state.tenant ||
    PILOT_TENANT
  );
}

/** Keep tenant fields in sync so Graph / Team / Status don't diverge mid-call. */
function syncTenantFields(t) {
  const v = (t || activeTenant()).trim() || PILOT_TENANT;
  state.tenant = v;
  for (const id of ["tenant", "team-tenant", "graph-tenant"]) {
    if ($(id) && $(id).value !== v) $(id).value = v;
  }
  return v;
}

async function jfetch(url, opts) {
  const res = await fetch(url, {
    headers: { "content-type": "application/json", ...(opts?.headers || {}) },
    ...opts,
  });
  const text = await res.text();
  let body;
  try {
    body = JSON.parse(text);
  } catch {
    body = { raw: text };
  }
  if (!res.ok) throw new Error(body.error || body.raw || res.statusText);
  return body;
}

function pill(name, up) {
  const cls = up === true ? "up" : up === false ? "down" : "mid";
  const label = up === true ? "up" : up === false ? "down" : "n/a";
  return `<span class="pill ${cls}">${name}: ${label}</span>`;
}

function fmtSecs(s) {
  if (s == null) return "—";
  if (s >= 86400) return `${Math.round(s / 86400)}d`;
  if (s >= 3600) return `${Math.round(s / 3600)}h`;
  if (s >= 60) return `${Math.round(s / 60)}m`;
  return `${s}s`;
}

/** Rewrite listed ISO instants to IST wall time. Never invents. */
function scrubListedTimes(s) {
  return String(s ?? "").replace(
    /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})/g,
    (m) => fmtIst(m)
  );
}

function softenInsightCopy(s) {
  return String(s ?? "")
    .replace(/\bPeak day\b/gi, "Busiest day")
    .replace(/\bUTC\b/g, "IST")
    .replace(/\bdigests\b/gi, "updates")
    .replace(/\bdigest\b/gi, "update")
    .replace(/\bChampion:\s*/g, "")
    .replace(/\bwork graph\b/gi, "map")
    .replace(/\bmulti-person status\b/gi, "team status");
}

/** Human age for last ingest (Connections health). */
function fmtAge(secs) {
  if (secs == null) return "no events yet";
  if (secs < 5) return "just now";
  if (secs < 60) return `${secs}s ago`;
  if (secs < 3600) return `${Math.round(secs / 60)}m ago`;
  if (secs < 86400) return `${Math.round(secs / 3600)}h ago`;
  return `${Math.round(secs / 86400)}d ago`;
}

function showView(name) {
  document.body.classList.toggle("view-graph-active", name === "graph");
  document.body.classList.toggle("view-cockpit-active", name === "cockpit");
  document.querySelectorAll(".view").forEach((el) => el.classList.add("hidden"));
  document.querySelectorAll(".nav-item").forEach((el) => el.classList.remove("active"));
  const view = $(`view-${name}`);
  if (view) view.classList.remove("hidden");
  const btn = document.querySelector(`.nav-item[data-view="${name}"]`);
  if (btn) btn.classList.add("active");
  const simple = isSimpleMode();
  const titles = {
    cockpit: [
      "Home",
      simple
        ? "What needs your attention — promises, status, and people"
        : "Same picture as Simple — denser layout, tags when useful",
    ],
    today: [
      "Today",
      simple ? "A quick read of the team" : "Pulse, updates, and open friction",
    ],
    status: [
      "My update",
      simple
        ? "Review and send — or hold it back"
        : "Approve, edit, or hold this draft",
    ],
    team: [
      "People",
      simple
        ? "Who’s on the team and how they’re doing"
        : "Map people, import, write updates",
    ],
    graph: [
      "Work map",
      simple
        ? "How people connect to the work"
        : "People, work, and focuses — drag, zoom, filter",
    ],
    connections: [
      "Connect",
      simple
        ? "Link chat and GitHub"
        : "Chat, GitHub, health, and install steps",
    ],
    settings: [
      "Settings",
      simple
        ? "How the product looks and how updates work"
        : "Look, cadence, health, and the activity trail",
    ],
    insights: [
      "Rhythm",
      "When the team is active — not a ranking",
    ],
    lab: [
      "Advanced",
      simple ? "Operator tools" : "Operator tools and raw payloads",
    ],
  };
  if (name === "cockpit") {
    refreshCockpit();
  }
  if (typeof applyChromeUxMode === "function") applyChromeUxMode();
  if (name === "connections") {
    refreshConnectors();
    refreshHealth();
    refreshOnboarding();
  }
  if (name === "team") {
    refreshTeam();
  }
  if (name === "insights") {
    refreshDevInsights();
    if (typeof applyInsightsUxMode === "function") applyInsightsUxMode();
  }
  if (name === "status") {
    loadLatest();
    if (typeof applyStatusUxMode === "function") applyStatusUxMode();
  }
  if (name === "today") {
    refreshPulse();
    refreshReadiness();
  }
  if (name === "graph") {
    startGraphView();
  } else {
    stopGraphLive();
  }
  if (name === "lab") {
    /* profile load is explicit */
  }
  const t = titles[name] || ["AI Manager", ""];
  $("view-title").textContent = t[0];
  $("view-sub").textContent = t[1];
}

async function refreshHealth() {
  try {
    const h = await jfetch("/v3/demo/status");
    state.status = h;
    const simple = isSimpleMode();
    $("conn-pills").innerHTML = simple
      ? [
          `<span class="pill up">App on</span>`,
          `<span class="pill ${h.v1 ? "up" : "down"}">${h.v1 ? "Work stream on" : "Work stream off"}</span>`,
          `<span class="pill ${h.v2 ? "up" : "down"}">${h.v2 ? "Map on" : "Map off"}</span>`,
          `<span class="pill ${h.egress ? "up" : "down"}">${h.egress ? "Chat delivery on" : "Chat delivery off"}</span>`,
        ].join("")
      : [
          pill("V3", true),
          pill("V1", h.v1),
          pill("V2", h.v2),
          pill("egress", h.egress),
        ].join("");
    const stackOk = h.v1 && h.v2;
    $("stat-stack").textContent = stackOk ? "Live" : "Partial";
    $("stat-stack-detail").textContent = stackOk
      ? simple
        ? "Work stream and map are reachable"
        : "V1 ingest + V2 graph reachable"
      : simple
        ? "Part of the stack is down — recover from Settings or wait for auto-heal."
        : "Start stack with ./scripts/dev_up.sh or docker compose -f deploy/docker-compose.app.yml up -d";
    $("stat-notify").textContent = fmtSecs(h.notify_interval_secs);
    $("stat-window").textContent = fmtSecs(h.status_window_secs);
    $("conn-detail").textContent = `Slack: ${h.slack_mode || "—"} · runtime ${h.mode || "—"} · notify ${h.notify_policy || "v1"}`;
    // Keep UX presentation mode in the foot (do not clobber with runtime mode)
    if ($("nav-mode")) {
      const ux = isSimpleMode() ? "Simple view" : "Technical view";
      $("nav-mode").textContent = ux;
    }
    $("cfg-window").textContent = fmtSecs(h.status_window_secs);
    $("cfg-notify").textContent = fmtSecs(h.notify_interval_secs);
    $("cfg-compile").textContent = fmtSecs(h.compile_interval_secs);
    $("cfg-noc").textContent = String(h.notify_on_compile_default ?? "—");
    $("cfg-slack").textContent = h.slack_mode || "—";

    // Connections: last successful ingest age (product health, not only process up)
    const ingestEl = $("conn-ingest");
    if (ingestEl) {
      if (!h.v1) {
        ingestEl.innerHTML = `<span class="pill down">GitHub / ingest: V1 down</span>`;
        if ($("conn-github")) {
          $("conn-github").textContent =
            "V1 not reachable. Start stack so webhooks and last-event age work.";
        }
      } else if (h.v1_last_event_age_secs == null) {
        ingestEl.innerHTML = [
          `<span class="pill mid">GitHub / ingest: up · no events yet</span>`,
          `<span class="pill mid">accepted: ${h.v1_accepted ?? 0}</span>`,
        ].join("");
        if ($("conn-github")) {
          $("conn-github").textContent =
            "V1 up. Waiting for first webhook or test ingest.";
        }
      } else {
        const age = h.v1_last_event_age_secs;
        const fresh = age < 600; // 10m
        ingestEl.innerHTML = [
          `<span class="pill ${fresh ? "up" : "mid"}">GitHub / ingest: last event ${fmtAge(age)}</span>`,
          `<span class="pill mid">accepted: ${h.v1_accepted ?? "—"}</span>`,
        ].join("");
        if ($("conn-github")) {
          $("conn-github").textContent = `Last event ${fmtAge(age)} · ${h.v1_accepted ?? 0} accepted this process.`;
        }
      }
    }
    // Graph durability (A3): never leave mystery empty looking "live"
    const graphEl = $("conn-graph");
    if (graphEl) {
      const gs = h.graph_status || (h.v2 ? "unknown" : "v2_down");
      const nodes = h.graph_nodes;
      const edges = h.graph_edges;
      if (gs === "v2_down" || !h.v2) {
        graphEl.innerHTML = `<span class="pill down">Graph: V2 down — recovering</span>`;
      } else if (gs === "empty" || nodes === 0) {
        graphEl.innerHTML = [
          `<span class="pill mid">Graph: empty (bridge re-projecting)</span>`,
          `<span class="pill mid">nodes 0</span>`,
        ].join("");
      } else if (typeof nodes === "number") {
        graphEl.innerHTML = [
          `<span class="pill up">Graph: filled</span>`,
          `<span class="pill mid">nodes ${nodes}</span>`,
          edges != null ? `<span class="pill mid">edges ${edges}</span>` : "",
        ]
          .filter(Boolean)
          .join("");
      } else {
        graphEl.innerHTML = `<span class="pill mid">Graph: ${esc(gs)}</span>`;
      }
    }
    if ($("conn-graph-detail")) {
      $("conn-graph-detail").textContent = isSimpleMode()
        ? "The map refills from recent work after a restart. Nothing important is only in memory."
        : (h.graph_message || "") +
          " Persistence: V1 events + ACL identity, V2 graph snapshot, V3 twins on disk (survive restarts).";
    }
    if ($("conn-slack")) {
      $("conn-slack").textContent = h.egress
        ? `Egress up · delivery mode: ${h.slack_mode || "—"}. Tokens only in vault. Notify Policy v1 (change-only + daily cap).`
        : "Egress down — real Slack DMs disabled until vault + proxy are up.";
    }
  } catch (e) {
    $("conn-pills").innerHTML = pill("V3", false);
    $("stat-stack").textContent = "Down";
    $("stat-stack-detail").textContent = String(e.message || e);
    const ingestEl = $("conn-ingest");
    if (ingestEl) ingestEl.innerHTML = "";
  }
}

function draftStatusLabel(st) {
  if (st === "vetoed") return "don't send";
  if (st === "publish_queued" || st === "PublishQueued") return "queued to share";
  if (st === "pending" || st === "Pending") return "pending approve";
  if (st === "edited" || st === "Edited") return "edited";
  if (st === "published" || st === "Published") return "shared";
  if (st === "shadow" || st === "Shadow") return "shadow (no DM)";
  if (st === "force_human" || st === "ForceHuman") return "needs human";
  return st || "?";
}

function renderEvidenceLine(refs) {
  const list = (refs || []).filter(Boolean);
  if (!list.length) return `<div class="muted small">evidence: (none linked)</div>`;
  return `<div class="muted small">evidence: ${list.map((r) => prettyMaybe(r)).join(" · ")}</div>`;
}

function renderLatest(payload) {
  if (!payload) return;
  state.latest = payload;
  state.tenant = $("tenant")?.value?.trim() || payload.draft?.tenant_id || state.tenant;
  state.draftId = payload.draft?.draft_id || null;
  state.ledgerId = payload.ledger_id || null;

  const conf = payload.confidence_rollup || payload.ledger?.confidence_rollup || "?";
  const st = payload.draft?.status || "?";
  const stLabel = draftStatusLabel(st);
  $("st-conf").textContent = `confidence: ${conf}`;
  $("st-conf").className = "pill " + (conf === "blocker" ? "down" : conf === "high" ? "up" : "mid");
  $("st-status").textContent = `draft: ${stLabel}`;
  $("st-status").className =
    "pill " + (st === "vetoed" ? "down" : st === "published" || st === "Published" ? "up" : "mid");
  if ($("st-ids")) {
    $("st-ids").innerHTML = [
      state.ledgerId ? prettyRef(state.ledgerId) : "",
      state.draftId ? prettyRef(state.draftId) : "",
    ]
      .filter(Boolean)
      .join(" ");
  }
  if ($("st-text")) {
    const rawText = payload.draft?.draft_text || "(no text)";
    $("st-text").dataset.raw = rawText;
    $("st-text").innerHTML = scrubTextHtml(rawText);
  }

  const items = payload.ledger?.items || [];
  const blockers = payload.ledger?.open_blockers || [];
  $("st-items").innerHTML = "";
  for (const it of items) {
    const li = document.createElement("li");
    li.innerHTML =
      `<strong>[${esc(it.confidence)}]</strong> ${scrubTextHtml(it.summary)}` +
      (it.resource_id ? ` ${prettyRef(it.resource_id)}` : "") +
      renderEvidenceLine(it.evidence_refs);
    $("st-items").appendChild(li);
  }
  for (const b of blockers) {
    const li = document.createElement("li");
    li.innerHTML =
      `<strong>[blocker]</strong> ${scrubTextHtml(b.summary)}` + renderEvidenceLine(b.evidence_refs);
    $("st-items").appendChild(li);
  }
  const emptyBanner = $("st-empty-banner");
  if (!items.length && !blockers.length) {
    $("st-items").innerHTML =
      "<li class='muted'>No items in this window — empty ledgers never Slack-DM (Notify Policy v1).</li>";
    if (emptyBanner) {
      emptyBanner.classList.remove("hidden");
      emptyBanner.innerHTML =
        "<strong>Empty status window.</strong> Nothing to approve. No DM was sent. Wait for real PR/commit/push activity, Team → Compile all, or send a test from Connections.";
    }
  } else if (emptyBanner) {
    emptyBanner.classList.add("hidden");
    emptyBanner.innerHTML = "";
  }

  $("today-latest").innerHTML = `
    <div class="meta-row">
      <span class="pill mid">confidence: ${esc(conf)}</span>
      <span class="pill mid">draft: ${esc(stLabel)}</span>
    </div>
    <pre class="box">${scrubTextHtml(payload.draft?.draft_text || "(no text)")}</pre>
  `;
  if ($("lab-raw")) $("lab-raw").textContent = JSON.stringify(payload, null, 2);
}

/** Load real pilot draft (team digests) — not lab simulate cache. */
async function openDraftById(tenant, draftId, ledgerId) {
  const t = syncTenantFields(tenant || activeTenant());
  if (!draftId) return false;
  try {
    const draft = await jfetch(
      `/v3/tenants/${encodeURIComponent(t)}/drafts/${encodeURIComponent(draftId)}`
    );
    let ledger = null;
    const lid = ledgerId || draft.ledger_id;
    if (lid) {
      try {
        ledger = await jfetch(
          `/v3/tenants/${encodeURIComponent(t)}/ledgers/${encodeURIComponent(lid)}`
        );
      } catch (_) {
        /* ledger optional */
      }
    }
    // Normalize shapes (API may return ledger nested or as ledger field)
    const ledgerBody = ledger?.ledger || ledger || {};
    const payload = {
      draft: draft.draft || draft,
      ledger_id: lid || draft.ledger_id || null,
      ledger: ledgerBody,
      confidence_rollup:
        ledgerBody.confidence_rollup ||
        draft.confidence_rollup ||
        ledger?.confidence_rollup ||
        "?",
    };
    // Ensure draft_id
    if (payload.draft && !payload.draft.draft_id) {
      payload.draft.draft_id = draftId;
    }
    renderLatest(payload);
    return true;
  } catch (e) {
    console.warn("openDraftById", e);
    return false;
  }
}

async function loadLatest() {
  const tenant = syncTenantFields(activeTenant());
  // 1) Prefer real team member drafts with content
  try {
    const team = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team`);
    const members = team.members || [];
    const withContent = members.find(
      (m) => m.last_digest?.has_content && m.last_digest?.draft_id
    );
    const anyDraft = members.find((m) => m.last_digest?.draft_id);
    const pick = withContent || anyDraft;
    if (pick?.last_digest?.draft_id) {
      const ok = await openDraftById(
        tenant,
        pick.last_digest.draft_id,
        pick.last_digest.ledger_id
      );
      if (ok) return;
    }
  } catch (_) {
    /* fall through */
  }
  // 2) Lab simulate cache (same tenant only)
  try {
    const payload = await jfetch(
      `/v3/demo/latest?tenant_id=${encodeURIComponent(tenant)}`
    );
    if (payload?.draft?.draft_id) {
      renderLatest(payload);
      return;
    }
  } catch {
    /* no snapshot */
  }
  // Empty state for sales path
  if ($("st-text")) {
    $("st-text").dataset.raw = "";
    $("st-text").textContent =
      "No draft yet for this tenant. Open Team → Compile all digests, or click a person on Today.";
  }
  if ($("st-items")) {
    $("st-items").innerHTML =
      "<li class='muted'>Compile team digests, then open a person here to Approve / Edit / Don't send.</li>";
  }
  state.draftId = null;
}

async function simulate() {
  $("btn-sim").disabled = true;
  $("btn-sim").textContent = "Sending…";
  try {
    const body = {
      tenant_id: syncTenantFields($("tenant")?.value || PILOT_TENANT),
      global_user_id: $("user").value.trim() || "gu_ec3cab86-2a3c-4737-bb04-d1f2deeae9f8",
      display_name: $("name").value.trim() || "neeljoshi18",
      slack_user_id: $("slack_user").value.trim() || "U0APK7W1X99",
      channel_id: $("channel").value.trim() || "C0APN754MQV",
      skip_shadow: true,
      pr_title: "Product UI test status",
    };
    const payload = await jfetch("/v3/demo/simulate", {
      method: "POST",
      body: JSON.stringify(body),
    });
    renderLatest(payload);
    showView("status");
  } catch (e) {
    alert("Test status failed: " + (e.message || e));
  } finally {
    $("btn-sim").disabled = false;
    $("btn-sim").textContent = "Send test status DM";
  }
}

async function act(kind) {
  if (!state.draftId) {
    alert(
      "No draft loaded.\n\nOpen a person from Cockpit / Team digests first, or click Compile digests.\n" +
        "If status is already “shared”, compile again for a new window before Approving."
    );
    return;
  }
  const base = `/v3/tenants/${encodeURIComponent(state.tenant)}/drafts/${encodeURIComponent(state.draftId)}`;
  const labels = { publish: "Approve", veto: "Don't send", edit: "Edit" };
  const label = labels[kind] || kind;
  const help = $("st-actions-help");
  if (help) help.textContent = `${label}…`;
  try {
    if (kind === "edit") {
      const text = prompt(
        "Edited status text:",
        $("st-text")?.dataset?.raw || $("st-text")?.textContent || ""
      );
      if (text == null) {
        if (help) help.textContent = "";
        return;
      }
      await jfetch(base + "/edit", { method: "POST", body: JSON.stringify({ text }) });
    } else {
      const body = await jfetch(base + "/" + kind, { method: "POST", body: "{}" });
      const outcome = body.outcome || body.draft?.status || "";
      const where = body.where_it_went || body.publish?.channel_id || "";
      const note = body.note || "";
      if (kind === "publish") {
        alert(
          (outcome === "already_published"
            ? "Already approved earlier.\n\n"
            : "Approved.\n\n") +
            (note ? note + "\n\n" : "") +
            (where ? "Where it went: " + where + "\n\n" : "") +
            "What this means:\n" +
            "• Digest status → published (twin store + event log)\n" +
            "• Shared to Slack channel if bot is a member, else DM fallback\n" +
            "• Work graph (commits/PRs) always from GitHub — not gated by Approve\n" +
            "• Graph shows a StatusDigest “approved” node (refresh Graph)\n" +
            "• Live trail: Connections/Settings events or GET /v3/tenants/…/events"
        );
      } else if (kind === "veto") {
        alert(
          "Don't send recorded.\n\n" +
            (note ? note + "\n\n" : "") +
            "Draft never posts to the team channel.\n" +
            "It stays as metadata. On Graph, check “Show unapproved digests” to see don't-send nodes."
        );
      }
    }
    // Reload same draft after action
    await openDraftById(state.tenant, state.draftId, state.ledgerId);
    if (help) {
      const st = state.latest?.draft?.status || "";
      help.innerHTML =
        `<strong>Last action:</strong> ${esc(label)} → draft is <code>${esc(st)}</code>. ` +
        `Approve shares status; Don't send keeps it as hidden metadata until you show unapproved on Graph.`;
    }
  } catch (e) {
    const msg = String(e.message || e);
    if (kind === "publish" && /not_in_channel|channel_not_found|invite the bot/i.test(msg)) {
      alert(
        "Approve: bot is not in the team Slack channel.\n\n" +
          "In Slack: invite @AI Manager (or your bot) to the team status channel, then retry.\n" +
          "Or use the latest build which falls back to a DM confirmation.\n\n" +
          msg
      );
    } else if (kind === "publish" && /egress|502|BAD_GATEWAY|proxy/i.test(msg)) {
      alert(
        "Approve failed — Slack delivery proxy is down or the bot token is missing.\n\n" +
          "Ops: recover egress (vault SLACK_BOT_TOKEN) and retry Approve.\n\n" +
          msg
      );
    } else {
      alert(label + " failed: " + msg);
    }
    if (help) help.textContent = label + " failed: " + msg;
  }
}

async function refreshReadiness() {
  const el = $("today-readiness");
  if (!el) return;
  const tenant = syncTenantFields(activeTenant());
  try {
    const r = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/pilot_readiness`
    );
    const soft = r.soft_outreach_ready === true;
    const multi = r.multi_person_ready === true;
    const content = r.content_people ?? 0;
    const a2 = r.checklist?.A2_multi_person_digests?.ok;
    const simple = isSimpleMode();
    el.innerHTML = simple
      ? [
          `<span class="pill ${soft || a2 ? "up" : "mid"}">${soft || a2 ? "Ready to show" : "Fine for one person"}</span>`,
          `<span class="pill ${multi ? "up" : "mid"}">${multi ? "Team of 2+ mapped" : "Add another person"}</span>`,
          `<span class="pill ${content >= 2 ? "up" : content >= 1 ? "mid" : "down"}">${content} update${content === 1 ? "" : "s"} with a story</span>`,
        ].join(" ")
      : [
          `<span class="pill ${soft || a2 ? "up" : "mid"}">sales: ${soft || a2 ? "ready" : "solo ok"}</span>`,
          `<span class="pill ${multi ? "up" : "mid"}">multi-person: ${multi ? "yes" : "no"}</span>`,
          `<span class="pill ${content >= 2 ? "up" : content >= 1 ? "mid" : "down"}">updates with content: ${content}</span>`,
          `<span class="pill mid">${esc(r.note || "").slice(0, 80)}</span>`,
        ].join(" ");
  } catch (e) {
    el.innerHTML = `<span class="pill mid">readiness: ${esc(e.message || "n/a")}</span>`;
  }
}

/** Champion cockpit — packages readiness, pod, conflicts, heat, graph, tomorrow focus. */
async function refreshCockpit() {
  const tenant = syncTenantFields(activeTenant());
  const msg = $("ck-msg");
  if (msg) msg.textContent = "Refreshing cockpit…";
  try {
    const [ready, team, pulse, ins, graph] = await Promise.all([
      jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/pilot_readiness`).catch((e) => ({
        error: e.message,
      })),
      jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team`).catch(() => ({ members: [] })),
      jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/pulse?refresh=true`
      ).catch(() => ({ conflicts: { cards: [] }, intents: {} })),
      jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/insights/dev`).catch(() => null),
      jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/graph?node_limit=200&edge_limit=400&include_demo=false`
      ).catch(() => null),
    ]);
    // Insights + commitments + technical ledger (non-blocking)
    loadPlainInsights().catch(() => {});
    loadCommitments("team").catch(() => {});
    loadIntentLedger().catch(() => {});

    const simple = isSimpleMode();
    // Readiness
    const soft = ready.soft_outreach_ready === true;
    const multi = ready.multi_person_ready === true || team.multi_person_ready === true;
    const content = ready.content_people ?? 0;
    if ($("ck-readiness")) {
      $("ck-readiness").innerHTML = simple
        ? [
            `<span class="pill ${soft ? "up" : "mid"}">${soft ? "Ready for a sales demo" : "OK for one-person pilot"}</span>`,
            `<span class="pill ${multi ? "up" : "mid"}">${multi ? "Multiple people mapped" : "Need 2+ people mapped"}</span>`,
            `<span class="pill ${content >= 2 ? "up" : content >= 1 ? "mid" : "down"}">${content} status update${content === 1 ? "" : "s"} with real content</span>`,
          ].join(" ")
        : [
            `<span class="pill ${soft ? "up" : "mid"}">soft_outreach: ${soft ? "ready" : "solo ok"}</span>`,
            `<span class="pill ${multi ? "up" : "mid"}">multi_person: ${multi ? "yes" : "need ≥2"}</span>`,
            `<span class="pill ${content >= 2 ? "up" : content >= 1 ? "mid" : "down"}">content_people: ${content}</span>`,
            `<span class="pill mid">${esc((ready.note || ready.error || "").toString().slice(0, 100))}</span>`,
          ].join(" ");
    }
    // Data flywheel strip
    if ($("ck-flywheel")) {
      try {
        const st = await jfetch("/v3/demo/status");
        const commits =
          (ins && ins.graph && ins.graph.commit_nodes) ||
          (ins && ins.activity && ins.activity.authored_edges) ||
          0;
        const gNodes = st.graph_nodes || (graph && (graph.nodes || []).length) || 0;
        const v1up = st.v1 === true;
        const v2up = st.v2 === true;
        if (simple) {
          $("ck-flywheel").innerHTML = [
            `<span class="pill ${v1up && v2up ? "up" : "down"}">${v1up && v2up ? "Work stream healthy" : "Work stream needs a check"}</span>`,
            `<span class="pill ${commits > 0 ? "up" : "mid"}">${commits} work updates on the map</span>`,
            `<span class="pill mid">${gNodes} people & work items linked</span>`,
            `<span class="pill ${st.egress ? "up" : "down"}">${st.egress ? "Chat delivery on" : "Chat delivery off"}</span>`,
          ].join(" ");
          if ($("ck-flywheel-note")) {
            $("ck-flywheel-note").textContent = v1up
              ? "Status writes itself from real work. You review digests and open loops — no standup theater."
              : "Work intake is down — status may be stale until the stack is recovered.";
          }
        } else {
          $("ck-flywheel").innerHTML = [
            `<span class="pill ${v1up ? "up" : "down"}">ingest V1: ${v1up ? "up" : "down"}</span>`,
            `<span class="pill ${v2up ? "up" : "down"}">graph V2: ${v2up ? "up" : "down"}</span>`,
            `<span class="pill ${commits >= 20 ? "up" : commits > 0 ? "mid" : "down"}">commits mapped: ${commits}</span>`,
            `<span class="pill mid">graph nodes: ${gNodes}</span>`,
            `<span class="pill ${st.egress ? "up" : "down"}">egress: ${st.egress ? "up" : "down"}</span>`,
          ].join(" ");
          if ($("ck-flywheel-note")) {
            $("ck-flywheel-note").textContent = v1up
              ? `Flywheel live — commits keep filling the graph (poller + webhooks).`
              : `Ingest V1 is down — recover stack so the flywheel does not stall.`;
          }
        }
      } catch (_) {
        /* non-fatal */
      }
    }

    const members = team.members || [];
    window.__ckMembers = members;
    const mapped = members.filter((m) => m.slack_mapped).length;
    const withContent = members.filter((m) => m.last_digest?.has_content).length;
    const cards = pulse.conflicts?.cards || [];
    const confCount = pulse.conflicts?.count ?? cards.length;

    if ($("ck-stat-mapped")) $("ck-stat-mapped").textContent = String(mapped);
    if ($("ck-stat-mapped-d"))
      $("ck-stat-mapped-d").textContent = simple
        ? `${mapped} people linked to chat`
        : `${members.length} twins · ${team.unique_slack_users ?? mapped} unique chat`;
    if ($("ck-stat-content")) $("ck-stat-content").textContent = String(withContent);
    if ($("ck-stat-content-d"))
      $("ck-stat-content-d").textContent = simple
        ? "people with a real status story"
        : `${members.filter((m) => m.last_digest).length} with any draft`;
    if ($("ck-stat-conflicts")) $("ck-stat-conflicts").textContent = String(confCount);
    if ($("ck-stat-conflicts-d"))
      $("ck-stat-conflicts-d").textContent = simple
        ? confCount > 0
          ? "places the team is stuck or mixed"
          : "no open friction right now"
        : confCount > 0
          ? "shared-work conflicts live"
          : "no open work conflicts";

    // Pod roster
    const pod = $("ck-pod");
    if (pod) {
      if (!members.length) {
        pod.innerHTML = simple
          ? `<li class="muted">No one on the pod yet — open <strong>Team</strong> and add people.</li>`
          : `<li class="muted">No pod members — open <strong>Team</strong> and map people (or bulk import).</li>`;
      } else if (simple) {
        pod.innerHTML = members
          .map((m) => {
            const d = m.last_digest;
            const name = m.display_name || "";
            const dig = plainDigestStatus(d);
            const preview = d?.preview
              ? d.preview
                  .split("\n")
                  .map((l) => l.replace(/^[*•\-\s]+/, "").trim())
                  .filter((l) => l && !l.match(/^[A-Z_]+:/) && !l.includes("led_") && !l.includes("dft_"))
                  .slice(0, 2)
                  .join(" · ")
                  .slice(0, 90)
              : "";
            const did = d?.draft_id || "";
            const lid = d?.ledger_id || "";
            const tone = d?.has_content ? "up" : d ? "mid" : "down";
            return `<li class="ux-card">
              <button type="button" class="ghost dig-open ux-card-btn" data-draft="${esc(did)}" data-ledger="${esc(lid)}">
                <span class="ux-avatar">${esc((name || "?").slice(0, 1).toUpperCase())}</span>
                <span class="ux-card-body">
                  <strong>${displayNameOrEye(name, m.subject_id)}</strong>
                  <span class="pill ${tone}">${esc(dig)}</span>
                  ${preview ? `<div class="muted small">${scrubTextHtml(preview)}</div>` : ""}
                  ${!m.slack_mapped ? `<div class="muted small">Chat not connected yet</div>` : ""}
                </span>
              </button>
            </li>`;
          })
          .join("");
      } else {
        pod.innerHTML = members
          .map((m) => {
            const d = m.last_digest;
            let dig = "no digest yet";
            if (d) {
              dig = d.has_content
                ? `${d.approx_item_count || "?"} item(s) · ${d.status_label || d.status}`
                : d.empty_placeholder
                  ? "empty window"
                  : d.status_label || d.status || "draft";
            }
            const did = d?.draft_id || "";
            const lid = d?.ledger_id || "";
            return `<li>
              <button type="button" class="ghost graph-filter-btn pod-row-btn dig-open"
                data-draft="${esc(did)}" data-ledger="${esc(lid)}">
                <strong>${displayNameOrEye(m.display_name, m.subject_id)}</strong>
                ${m.subject_id && m.subject_id !== m.display_name ? `<span class="muted small"> · ${prettyMaybe(m.subject_id)}</span>` : ""}
                ${m.slack_mapped ? "" : " · <span class='muted'>chat not linked</span>"}
              </button>
              <div class="muted small">${esc(dig)}${d?.preview ? " · " + scrubTextHtml(d.preview.slice(0, 72)) : ""}
              ${did ? ` · ${prettyRef(did)}` : ""}</div>
            </li>`;
          })
          .join("");
      }
      pod.querySelectorAll(".dig-open").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const did = btn.getAttribute("data-draft");
          const lid = btn.getAttribute("data-ledger");
          if (!did) {
            alert(simple ? "No status update yet — try Compile digests first" : "No draft yet — Compile digests first");
            return;
          }
          const ok = await openDraftById(tenant, did, lid);
          if (ok) showView("status");
        });
      });
    }

    // Conflicts / friction
    const confEl = $("ck-conflicts");
    if (confEl) {
      if (!cards.length) {
        const emptyWhy = pulse.conflicts?.empty_reason || "";
        const demoN = pulse.conflicts?.demo_count ?? 0;
        let emptyMsg = simple
          ? "No open friction right now. When people disagree on ship vs hold, it shows up here."
          : "No open live conflicts (empty_reason=no_friction). Organic PRs feed this surface — not demo seeds.";
        if (emptyWhy === "only_demo_seeds" || demoN > 0) {
          emptyMsg = simple
            ? "No live friction right now. Example stories stay hidden until two people disagree on real work."
            : `No live conflicts (empty_reason=only_demo_seeds, demo_count=${demoN}). Seeds excluded from primary cards.`;
        }
        confEl.innerHTML = `<p class="muted">${esc(emptyMsg)}</p>`;
      } else if (simple) {
        confEl.innerHTML =
          `<div class="ux-friction-list">` +
          cards
            .slice(0, 12)
            .map((c) => {
              const title = plainConflictKind(c.kind);
              const sev = plainSeverity(c.severity);
              const sum = (c.summary || "")
                .replace(/intent:person:[^\s]+/gi, "")
                .replace(/pr:[^\s]+/gi, "a shared pull request")
                .replace(/cfl_[^\s]+/gi, "")
                .trim();
              return `<div class="ux-friction-card">
                <div class="ux-friction-top">
                  <span class="pill ${c.severity === "high" ? "down" : "mid"}">${esc(sev || "Note")}</span>
                  <strong>${esc(title)}</strong>
                </div>
                <p class="muted small">${esc(sum || "Needs a quick alignment conversation.")}</p>
                <p class="muted small"><em>What to do:</em> Get the people involved in one thread and pick a direction.</p>
              </div>`;
            })
            .join("") +
          `</div>`;
      } else {
        confEl.innerHTML =
          `<ul class="item-list">` +
          cards
            .slice(0, 12)
            .map(
              (c) =>
                `<li><strong>[${esc(c.severity || c.kind)}]</strong> ${esc(c.summary || c.kind)} <span class="muted small">${esc(c.kind || "")}</span></li>`
            )
            .join("") +
          `</ul>`;
      }
    }
    const intentUl = $("ck-intents");
    if (intentUl) {
      const sample = pulse.intents?.sample || [];
      if (!sample.length) {
        intentUl.innerHTML = `<li class="muted">${simple ? "No open focuses in the sample" : "No live intents in sample"}</li>`;
      } else if (simple) {
        intentUl.innerHTML = sample
          .slice(0, 12)
          .map((n) => {
            const ty = n.intent_type || n.type || "Intent";
            const lab = (n.label || n.title || "")
              .replace(/^(SHIP|BLOCKED|FREEZE|FIX|EXPLORE|REVIEW):\s*/i, "");
            return `<li class="ux-chip-row"><span class="pill mid">${esc(plainIntentType(ty))}</span> ${esc(lab || "Work in flight")}</li>`;
          })
          .join("");
      } else {
        intentUl.innerHTML = sample
          .slice(0, 12)
          .map((n) => {
            const ty = n.intent_type || n.type || n.properties?.intent_type || "Intent";
            const lab = (n.label || n.title || n.display_name || "")
              .replace(/^(SHIP|BLOCKED|FREEZE|FIX|EXPLORE|REVIEW):\s*/i, "");
            return `<li><span class="pill mid">${esc(ty)}</span> ${lab ? esc(lab.slice(0, 80)) : prettyRef(n.id || n.node_id || "")}</li>`;
          })
          .join("");
      }
    }

    // Heat
    if (ins && ins.activity) {
      const act = ins.activity;
      if ($("ck-heat-insight")) {
        $("ck-heat-insight").textContent = simple
          ? act.insight
              ? softenInsightCopy(act.insight)
              : "When the team usually ships work (IST)."
          : softenInsightCopy(act.insight || "");
      }
      const hod = act.hour_of_day_ist || act.hour_of_day_utc || {};
      const counts = hod.counts || [];
      const labels = hod.labels || [];
      if ($("ck-heat-hours")) {
        $("ck-heat-hours").innerHTML = heatBarsHtml(counts, labels);
      }
      if ($("ck-heat-authors")) {
        const by = act.by_author || {};
        const top = Object.entries(by)
          .sort((a, b) => b[1] - a[1])
          .slice(0, 6);
        $("ck-heat-authors").innerHTML = top.length
          ? (simple
              ? `Most active on the map: ${top.map(([k, v]) => `${prettyMaybe(k)} (${v})`).join(", ")} — context only, not a ranking.`
              : `Authored volume (context, not rank): ${top.map(([k, v]) => `${prettyMaybe(k)}: ${v}`).join(" · ")}`)
          : "";
      }
    } else if ($("ck-heat-insight")) {
      $("ck-heat-insight").textContent = simple ? "Rhythm data not available yet" : "Heat unavailable";
    }

    // Graph stats
    if ($("ck-graph-stats") && graph) {
      const nodes = (graph.nodes || []).length;
      const edges = (graph.edges || []).length;
      const by = graph.by_type || {};
      const friendly = {
        Commit: "code updates",
        Person: "people",
        Repo: "projects",
        PullRequest: "reviews",
        Intent: "focuses",
        Issue: "issues",
      };
      const bits = Object.entries(by)
        .map(([k, v]) => `<span class="pill mid">${v} ${esc(friendly[k] || k.toLowerCase())}</span>`)
        .join(" ");
      $("ck-graph-stats").innerHTML =
        `<div class="meta-row" style="margin:0;">` +
        `<span class="pill up">${nodes} items</span>` +
        `<span class="pill mid">${edges} links</span>` +
        bits +
        `</div>`;
    }

    // Tomorrow focus — suggestions from conflicts, intents, digests + persisted pins
    const tomorrow = [];
    for (const c of cards.slice(0, 5)) {
      tomorrow.push({
        kind: "conflict",
        text: simple
          ? `${plainConflictKind(c.kind)}: ${(c.summary || "").replace(/intent:person:\S+/g, "").replace(/pr:\S+/g, "shared work").trim() || "needs alignment"}`
          : `${c.severity || c.kind}: ${c.summary || c.kind}`,
        why: simple
          ? "Clear this before the next team sync"
          : "Resolve shared-work conflict before next standup",
        pinned: false,
      });
    }
    for (const n of (pulse.intents?.sample || []).slice(0, 5)) {
      const ty = n.intent_type || "Intent";
      if (ty === "BLOCKED" || ty === "FREEZE" || ty === "SHIP") {
        const lab = (n.label || n.title || "").replace(/^(SHIP|BLOCKED|FREEZE|FIX):\s*/i, "");
        tomorrow.push({
          kind: "intent",
          text: simple ? `${plainIntentType(ty)} — ${lab}` : `${ty}: ${n.label || n.title || ""}`,
          why: simple ? "Someone needs a clear next step" : "Intent needs champion attention",
          pinned: false,
        });
      }
    }
    for (const m of members) {
      if (m.last_digest?.has_content && m.last_digest?.preview) {
        const line = (m.last_digest.preview || "").split("\n").find((l) => l.includes("•"));
        if (line) {
          tomorrow.push({
            kind: "digest",
            text: `${m.display_name}: ${line.replace(/^[\s*•]+/, "").slice(0, 90)}`,
            why: "From their latest status draft",
            pinned: false,
          });
        }
      }
    }
    let pinnedItems = [];
    try {
      const foc = await jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/tomorrow_focus`
      );
      pinnedItems = foc.focus?.items || [];
    } catch (_) {
      /* optional */
    }
    for (const p of pinnedItems) {
      tomorrow.unshift({
        kind: p.kind || "pin",
        text: p.text || p.title || JSON.stringify(p),
        why: p.why || "Pinned assignment",
        pinned: true,
      });
    }
    // de-dupe by text
    const seenT = new Set();
    const uniq = [];
    for (const t of tomorrow) {
      const k = t.text.slice(0, 80);
      if (seenT.has(k)) continue;
      seenT.add(k);
      uniq.push(t);
    }
    window.__ckTomorrowUniq = uniq;
    const tomEl = $("ck-tomorrow");
    if (tomEl) {
      tomEl.innerHTML = uniq.length
        ? uniq
            .slice(0, 12)
            .map(
              (t) =>
                `<li><span class="pill ${t.pinned ? "up" : "mid"}">${esc(t.pinned ? "Pinned" : t.kind)}</span> <strong>${scrubTextHtml(t.text)}</strong><div class="muted small">${esc(t.why)}</div></li>`
            )
            .join("")
        : `<li class="muted">No suggestions yet — write status updates or enrich the story so open work appears.</li>`;
    }
    if ($("ck-tomorrow-note")) {
      const nPin = pinnedItems.length;
      $("ck-tomorrow-note").textContent = nPin
        ? `${nPin} pinned assignment(s) persisted for this tenant · suggestions merge above.`
        : "Pin the board to persist champion focus across reloads (tenant state).";
    }

    // Visual simple home (always fill; visibility via CSS)
    await fillVisualHome({
      tenant,
      members,
      mapped,
      withContent,
      confCount,
      cards,
      ins,
      soft,
      multi,
    });

    if (msg) {
      msg.textContent = simple
        ? `Updated just now`
        : `Updated ${nowIstList()} · tenant ${tenant}`;
    }
  } catch (e) {
    if (msg) msg.textContent = "Cockpit failed: " + (e.message || e);
  }
}

/** Simple-home promise filter: team | mine | owed (same commitment API as Technical). */
let __vizCmtFilter = "team";

/** Simple-mode visual board — org story without ops jargon (full feature surface). */
async function fillVisualHome(ctx) {
  const {
    tenant,
    members = [],
    mapped = 0,
    withContent = 0,
    confCount = 0,
    cards = [],
    ins = null,
    soft = false,
    multi = false,
  } = ctx || {};
  let insights = null;
  let cmts = [];
  let claims = [];
  const subject =
    $("ck-profile-subject")?.value?.trim() ||
    members[0]?.display_name ||
    "neeljoshi18";

  // Highlight active filter chips
  document.querySelectorAll(".viz-filter").forEach((btn) => {
    const f = btn.getAttribute("data-filter");
    btn.classList.toggle("active", f === __vizCmtFilter);
  });

  try {
    insights = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/intent/insights`);
  } catch (_) {}
  try {
    let q = "status=open&limit=20";
    if (__vizCmtFilter === "mine") q += `&i_owe=${encodeURIComponent(subject)}`;
    if (__vizCmtFilter === "owed") q += `&owed_to=${encodeURIComponent(subject)}`;
    const body = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/commitments?${q}`
    );
    cmts = body.commitments || [];
  } catch (_) {}
  try {
    const led = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/intent/ledger?include_demo=false&open_only=true&limit=20`
    );
    claims = led.claims || [];
  } catch (_) {}

  const act = insights?.act_on_today || [];
  const wins = insights?.good_news || [];
  const needN = act.length + confCount;
  const openPromises = cmts.filter((c) => c.status === "open").length;
  const openFocus = claims.length;

  if ($("viz-m-people")) $("viz-m-people").textContent = String(mapped || members.length || 0);
  if ($("viz-m-updates")) $("viz-m-updates").textContent = String(withContent);
  if ($("viz-m-attention")) $("viz-m-attention").textContent = String(needN);
  if ($("viz-m-promises")) $("viz-m-promises").textContent = String(openPromises);

  // Headline story
  let headline = "Your team is quiet right now.";
  if (needN > 0 && openPromises > 0) {
    headline = `${needN} thing${needN === 1 ? "" : "s"} need you · ${openPromises} open promise${openPromises === 1 ? "" : "s"}.`;
  } else if (needN > 0) {
    headline = `${needN} thing${needN === 1 ? "" : "s"} need your attention today.`;
  } else if (openPromises > 0) {
    headline = `${openPromises} open promise${openPromises === 1 ? "" : "s"} to keep an eye on.`;
  } else if (openFocus > 0) {
    headline = `${openFocus} open focus item${openFocus === 1 ? "" : "s"} on the board.`;
  } else if (withContent >= 2) {
    headline = "Status is flowing — no fires in the open loops.";
  } else if (mapped >= 2) {
    headline = "People are mapped. Write status updates when you're ready.";
  } else if (mapped < 2) {
    headline = "Add people under People, then connect chat.";
  }
  if ($("viz-headline")) $("viz-headline").textContent = headline;
  if ($("viz-sub")) {
    $("viz-sub").textContent = soft || multi
      ? "Promises, focuses, and status in one place. You approve what gets sent."
      : "Connect chat and GitHub, add your people, then this board fills itself.";
  }

  // Attention cards
  const att = $("viz-attention");
  if (att) {
    const items = [];
    for (const a of act.slice(0, 6)) {
      items.push({
        title: softenInsightCopy(a.text || "Something needs a look"),
        action: softenInsightCopy(a.action || "Open the thread and decide next step"),
        tone: a.priority === "high" ? "urgent" : "soon",
        cmtId: a.kind === "commitment" ? a.id : null,
      });
    }
    for (const c of cards.slice(0, 4)) {
      if (c.is_demo) continue;
      items.push({
        title: plainConflictKind(c.kind),
        action: "Get the people involved and pick ship vs hold — or reassign ownership.",
        tone: c.severity === "high" ? "urgent" : "soon",
        cmtId: null,
      });
    }
    if (!items.length) {
      att.innerHTML = `<div class="am-empty">Nothing urgent. Enjoy the quiet.</div>`;
    } else {
      att.innerHTML = items
        .slice(0, 6)
        .map((it) =>
          amRow({
            title: esc(it.title),
            meta: esc(it.action),
            tone: it.tone === "urgent" ? "urgent" : "soon",
            actionsHtml: it.cmtId
              ? `<button type="button" class="ghost viz-done" data-id="${esc(it.cmtId)}">Mark done</button>`
              : "",
          })
        )
        .join("");
      att.querySelectorAll(".viz-done").forEach((btn) => {
        btn.addEventListener("click", async () => {
          try {
            await jfetch(
              `/v3/tenants/${encodeURIComponent(tenant)}/commitments/${encodeURIComponent(btn.getAttribute("data-id"))}/done`,
              { method: "POST", body: "{}" }
            );
            refreshCockpit();
          } catch (e) {
            alert(e.message || e);
          }
        });
      });
    }
  }

  // Promises (full loop: done + dismiss — same APIs as Technical)
  const pr = $("viz-promises");
  if (pr) {
    if (!cmts.length) {
      const filterHint =
        __vizCmtFilter === "mine"
          ? "Nothing you currently owe. Switch to Team or add a promise."
          : __vizCmtFilter === "owed"
            ? "Nothing owed to you right now."
            : "No open promises. When someone says “I'll…”, it lands here — or tap Add a promise.";
      pr.innerHTML = `<div class="am-empty">${esc(filterHint)}</div>`;
    } else {
      pr.innerHTML = cmts
        .slice(0, 8)
        .map((c) => {
          const who = c.promiser_label || c.promiser || "Someone";
          const to = c.promisee_label || c.promisee;
          return amRow({
            title: esc(c.headline || c.text || ""),
            meta: `${humanOwnerHtml(who)}${to ? " → " + humanOwnerHtml(to) : ""}`,
            tone: "open",
            actionsHtml: `<button type="button" class="primary viz-done" data-id="${esc(c.id)}">Done</button>
              <button type="button" class="ghost viz-dismiss" data-id="${esc(c.id)}">Not doing</button>`,
          });
        })
        .join("");
      pr.querySelectorAll(".viz-done").forEach((btn) => {
        btn.addEventListener("click", async () => {
          try {
            await jfetch(
              `/v3/tenants/${encodeURIComponent(tenant)}/commitments/${encodeURIComponent(btn.getAttribute("data-id"))}/done`,
              { method: "POST", body: "{}" }
            );
            refreshCockpit();
          } catch (e) {
            alert(e.message || e);
          }
        });
      });
      pr.querySelectorAll(".viz-dismiss").forEach((btn) => {
        btn.addEventListener("click", async () => {
          try {
            await jfetch(
              `/v3/tenants/${encodeURIComponent(tenant)}/commitments/${encodeURIComponent(btn.getAttribute("data-id"))}/dismiss`,
              { method: "POST", body: "{}" }
            );
            refreshCockpit();
          } catch (e) {
            alert(e.message || e);
          }
        });
      });
    }
  }

  // Open focuses (intent ledger — plain English)
  const focusEl = $("viz-focus");
  if (focusEl) {
    if (!claims.length) {
      focusEl.innerHTML = `<div class="am-empty">No open focuses. Capture one, or wait for work to surface them.</div>`;
    } else {
      focusEl.innerHTML = claims
        .slice(0, 8)
        .map((c) => {
          const ty = plainIntentType(c.intent_type || "OTHER");
          const sum = (c.summary || c.text_preview || "")
            .replace(/^(SHIP|BLOCKED|FREEZE|FIX|EXPLORE|REVIEW):\s*/i, "")
            .slice(0, 140);
          const own = humanOwnerHtml(
            c.owner_label || c.owner_display || c.owner_subject || "",
            members
          );
          const tone =
            c.intent_type === "BLOCKED" || c.intent_type === "FREEZE" ? "urgent" : "open";
          return amRow({
            title: `${esc(sum || ty)}`,
            meta: `<span class="pill mid">${esc(ty)}</span> ${own || "Someone"}`,
            tone,
          });
        })
        .join("");
    }
  }

  // Good news
  const winsEl = $("viz-wins");
  if (winsEl) {
    if (!wins.length) {
      winsEl.innerHTML = `<div class="am-empty">Good news shows up as work and status land.</div>`;
    } else {
      winsEl.innerHTML = wins
        .slice(0, 5)
        .map((w) =>
          amRow({
            title: esc(softenInsightCopy(w.text || "Progress")),
            meta: esc(softenInsightCopy(w.action || "")),
            tone: "ok",
          })
        )
        .join("");
    }
  }

  // People grid
  const pg = $("viz-people-grid");
  if (pg) {
    if (!members.length) {
      pg.innerHTML = `<div class="am-empty">No people yet — open People and add the team.</div>`;
    } else {
      pg.innerHTML = members
        .map((m) => {
          const name = m.display_name || "";
          const dig = plainDigestStatus(m.last_digest);
          const ok = m.last_digest?.has_content;
          const did = m.last_digest?.draft_id || "";
          const lid = m.last_digest?.ledger_id || "";
          return `<button type="button" class="viz-person dig-open" data-draft="${esc(did)}" data-ledger="${esc(lid)}">
            <span class="ux-avatar">${esc((name || "?").slice(0, 1).toUpperCase())}</span>
            <span class="viz-person-name">${displayNameOrEye(name, m.subject_id)}</span>
            <span class="pill ${ok ? "up" : "mid"}">${esc(dig)}</span>
          </button>`;
        })
        .join("");
      pg.querySelectorAll(".dig-open").forEach((btn) => {
        btn.addEventListener("click", async () => {
          const did = btn.getAttribute("data-draft");
          if (!did) {
            alert("No status update yet for this person.");
            return;
          }
          const ok = await openDraftById(tenant, did, btn.getAttribute("data-ledger"));
          if (ok) showView("status");
        });
      });
    }
  }

  // Rhythm
  if ($("viz-rhythm")) {
    const insight = ins?.activity?.insight;
    $("viz-rhythm").textContent = insight
      ? softenInsightCopy(insight)
      : "Activity rhythm appears as the work map fills (IST).";
  }
  const bars = $("viz-rhythm-bars");
  const hodAct = ins?.activity?.hour_of_day_ist || ins?.activity?.hour_of_day_utc;
  if (bars && hodAct) {
    bars.innerHTML = heatBarsHtml(hodAct.counts || [], hodAct.labels || []);
  }
}

async function bulkImportTeam() {
  const tenant = syncTenantFields(activeTenant());
  const raw = $("tm-bulk")?.value || "";
  const msg = $("team-bulk-msg");
  const lines = raw
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);
  if (!lines.length) {
    if (msg) msg.textContent = "Paste at least one line.";
    return;
  }
  let ok = 0;
  let fail = 0;
  const notes = [];
  for (const line of lines) {
    // display | github | slack | optional subject
    const parts = line.split("|").map((s) => s.trim());
    if (parts.length < 3) {
      fail++;
      notes.push(`bad line: ${line.slice(0, 40)}`);
      continue;
    }
    const [display_name, github, slack_user_id, subjectOpt] = parts;
    const subject_id = subjectOpt || github || display_name;
    const provider_aliases = [github, display_name].filter(Boolean);
    try {
      await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team/members`, {
        method: "POST",
        body: JSON.stringify({
          subject_id,
          display_name: display_name || subject_id,
          slack_user_id,
          provider_aliases,
          skip_shadow: true,
          enabled: true,
        }),
      });
      ok++;
    } catch (e) {
      fail++;
      notes.push(`${display_name}: ${e.message || e}`);
    }
  }
  if (msg) {
    msg.textContent = `Imported ${ok} ok, ${fail} failed. ${notes.slice(0, 3).join(" · ")}`;
  }
  await refreshTeam();
  if (!$("view-cockpit")?.classList.contains("hidden")) {
    await refreshCockpit();
  }
}

document.querySelectorAll(".nav-item").forEach((btn) => {
  btn.addEventListener("click", () => showView(btn.dataset.view));
});
async function refreshOnboarding() {
  const el = $("onboard-steps");
  if (!el) return;
  try {
    const o = await jfetch("/v3/onboarding/status");
    el.innerHTML = "";
    for (const step of o.steps || []) {
      const li = document.createElement("li");
      li.className = step.done ? "done" : "todo";
      const simple = isSimpleMode();
      const title = simple
        ? String(step.title || "")
            .replace(/Stack running/i, "System ready")
            .replace(/Egress vault/i, "Chat delivery")
            .replace(/Shadow \/ batch notify/i, "Quiet updates")
            .replace(/First status digests/i, "First updates sent")
            .replace(/Non-empty digests.*/i, "Updates with a story")
            .replace(/Map ≥2 people/i, "Team of 2+")
        : step.title;
      const detail = simple
        ? String(step.detail || "")
            .replace(/V1 \+ V2 reachable/i, "work stream and map are up")
            .replace(/tokens stay in vault only/i, "tokens stay locked away")
            .replace(/person twins on ten_github/i, "people on this workspace")
            .replace(/notify every \d+s[^.]*$/i, "writes only when something changes")
            .replace(/real digest content.*/i, "real status stories")
        : step.detail || "";
      li.innerHTML = `<span class="mark">${step.done ? "✓" : "○"}</span> <strong>${esc(title)}</strong> — <span class="muted">${esc(detail)}</span>`;
      el.appendChild(li);
    }
    if ($("onboard-note")) {
      $("onboard-note").textContent = isSimpleMode()
        ? "Connecting Slack and GitHub needs a one-time install in the other tab."
        : o.note || "";
    }
    // Enable OAuth buttons when server says ready (still may 501 if partial)
    const gh = $("btn-gh-app");
    const sl = $("btn-slack-oauth");
    if (gh) {
      gh.disabled = false;
      gh.title = o.github_app_ready
        ? "Open GitHub App install"
        : "Will show setup instructions until GITHUB_APP_ID is set";
    }
    if (sl) {
      sl.disabled = false;
      sl.title = o.slack_oauth_ready
        ? "Start Slack OAuth"
        : "Will show vault/manual path until SLACK_CLIENT_ID is set";
    }
  } catch (e) {
    el.innerHTML = `<li class="todo">Onboarding status unavailable: ${e.message || e}</li>`;
  }
}

/** Show / hide post-install callout on Connections (no secrets). */
function showPostInstallCallout(kind, detail) {
  const box = $("conn-post-install");
  const title = $("conn-post-install-title");
  const body = $("conn-post-install-body");
  if (!box || !title || !body) return;
  box.classList.remove("hidden");
  if (kind === "slack") {
    title.textContent = "Slack connected — next steps";
    const ch = $("channel")?.value?.trim();
    const chHint = ch
      ? ` Team channel on the form: <code>${esc(ch)}</code> — invite the bot there for channel posts.`
      : " Invite the bot to your team channel for channel posts.";
    body.innerHTML =
      `1) Paste the <strong>Events Request URL</strong> into Slack app → Event Subscriptions (copy below). Bot events: <code>message.channels</code>, <code>message.groups</code>, <code>message.im</code>.<br/>` +
      `2) ${chHint} Needed for ambient “blocked on…” / “I’ll send…” claims. <em>Digests still DM</em> mapped people if the bot is not in a channel.<br/>` +
      `3) Map your pod under <strong>Team</strong> (Slack user ids).<br/>` +
      `4) Open <strong>Cockpit</strong>. If digests fail after first connect, restart egress once so it reloads the vault. Reconnect Slack if this workspace predates channel-history scopes.` +
      (detail ? `<br/><span class="muted">${esc(detail)}</span>` : "");
  } else if (kind === "github") {
    title.textContent = "GitHub App — next steps";
    body.innerHTML =
      `1) Confirm webhook URL is set on the App (copy below).<br/>` +
      `2) Install on org/repos that should feed status.<br/>` +
      `3) Wait ~1 min, then open <strong>Graph / Cockpit</strong>. GitHub = work signals (not LOC rankings).` +
      (detail ? `<br/><span class="muted">${esc(detail)}</span>` : "");
  } else {
    title.textContent = "Install progress";
    body.textContent = detail || "Use Refresh status after finishing install in the other tab.";
  }
}

async function startOAuth(kind) {
  const path =
    kind === "slack"
      ? "/v3/oauth/slack/start"
      : kind === "teams"
        ? "/v3/oauth/teams/start"
        : "/v3/oauth/github/start";
  const guideEl = $("conn-guide");
  try {
    const res = await fetch(path);
    const body = await res.json().catch(() => ({}));
    if (body.ready === false || body.error) {
      const manual =
        body.manual_path ||
        body.webhook_url ||
        body.webhook_path ||
        body.messaging_endpoint ||
        "deploy/oauth/README.md";
      const msg =
        (body.message || "Not fully configured") +
        "\n\nManual path:\n" +
        manual +
        (body.webhook_url ? "\n\nWebhook URL:\n" + body.webhook_url : "") +
        (body.messaging_endpoint
          ? "\n\nTeams messaging endpoint:\n" + body.messaging_endpoint
          : "");
      alert(msg);
      if (guideEl) {
        guideEl.innerHTML = `<strong>Setup needed.</strong> ${esc(body.message || "Not configured")}. Manual: <code>${esc(manual)}</code>`;
      }
      if (body.webhook_url && $("conn-gh-webhook")) {
        $("conn-gh-webhook").textContent = body.webhook_url;
      }
      // Still open install URL if present (e.g. GH slug without full env)
      if (body.install_url) {
        const w = window.open(body.install_url, "_blank", "noopener");
        if (!w && guideEl) {
          guideEl.innerHTML += ` <a class="primary linkbtn" href="${esc(body.install_url)}" target="_blank" rel="noopener">Open install link (popup blocked)</a>`;
        }
      }
      return;
    }
    const url = body.authorize_url || body.install_url;
    if (!url) {
      alert(JSON.stringify(body, null, 2));
      return;
    }
    // Always surface webhook when GitHub responds
    if (kind === "github" && body.webhook_url && $("conn-gh-webhook")) {
      $("conn-gh-webhook").textContent = body.webhook_url;
    }
    // Pre-flight: what will happen
    if (kind === "slack") {
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Connect Slack</strong> (delivery) — install the AI Manager bot on your workspace. ` +
          `After Allow you land on “Slack connected”, then return here. Digests use the bot token (vault only). ` +
          `Then paste the Events URL and invite the bot to the team channel for ambient claims. ` +
          `Restart egress once after first connect so it reloads secrets.`;
      }
      const ok = confirm(
        "Connect Slack (delivery)\n\n" +
          "1. Slack will open — install the bot on your workspace\n" +
          "2. Approve delivery + channel-history scopes (Events / ambient claims)\n" +
          "3. You’ll land on “Slack connected” with next steps\n" +
          "4. Return here → paste Events URL → invite bot to channel → map Team → Cockpit\n\n" +
          "Continue to Slack?"
      );
      if (!ok) return;
    } else if (kind === "github") {
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Install GitHub App</strong> (work signals) — pick the org/repos that should feed status. ` +
          `Webhooks hit <code>${esc(body.webhook_url || "…/webhooks/github")}</code>. ` +
          `Graph fills via V1→bridge→V2. Not LOC rankings.`;
      }
      const ok = confirm(
        "Install GitHub App (work signals)\n\n" +
          "1. GitHub will open the App install page\n" +
          "2. Choose org + repositories for status\n" +
          "3. Webhooks post to the product host automatically\n" +
          "4. Return here — click “I finished install — refresh”, then Graph / Cockpit\n\n" +
          "Continue to GitHub?"
      );
      if (!ok) return;
    } else if (kind === "teams") {
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Connect Teams</strong> (secondary — default stays Slack) — Azure Bot + Adaptive Cards. ` +
          `Needs TEAMS_APP_ID + vault TEAMS_BOT_TOKEN. ` +
          `Messaging endpoint: <code>${esc(body.messaging_endpoint || "")}</code>`;
      }
    }
    const win = window.open(url, "_blank", "noopener");
    if (!win) {
      // Popup blocked: in-page recovery so the path never looks broken
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Popup blocked.</strong> ` +
          `<a class="primary linkbtn" href="${esc(url)}" target="_blank" rel="noopener">Open ${esc(kind)} install in this tab →</a> ` +
          `<span class="muted">or allow popups for this host and try again.</span>`;
      }
      const go = confirm(
        "Popup blocked by the browser.\n\n" +
          "OK = open the install URL in this tab.\n" +
          "Cancel = copy the URL from the Connections guide.\n\n" +
          url
      );
      if (go) {
        window.location.href = url;
        return;
      }
      try {
        await navigator.clipboard.writeText(url);
        if (guideEl) guideEl.innerHTML += ` <span class="muted">(URL copied)</span>`;
      } catch {
        prompt("Copy install URL:", url);
      }
    } else {
      // Other tab opened — keep champion oriented
      if (kind === "slack") {
        showPostInstallCallout("slack", "Finish Allow in the Slack tab, then click Refresh status.");
      } else if (kind === "github") {
        showPostInstallCallout("github", "Finish install in the GitHub tab, then click Refresh status.");
      }
      if (guideEl) {
        guideEl.innerHTML +=
          ` <span class="muted">Install opened in another tab. When done, click <strong>I finished install — refresh status</strong>.</span>`;
      }
    }
  } catch (e) {
    alert("OAuth start failed: " + (e.message || e));
  }
}

/** Refresh Connectors panel + oauth pills + checklist (Connections). */
async function refreshConnectors() {
  const statusEl = $("conn-oauth-status");
  try {
    const [o, demo] = await Promise.all([
      jfetch("/v3/oauth/status"),
      jfetch("/v3/demo/status").catch(() => null),
    ]);
    const slack = o.slack || {};
    const gh = o.github || {};
    const teams = o.teams || {};
    const checklist = Array.isArray(o.install_checklist) ? o.install_checklist : [];
    const byId = Object.fromEntries(checklist.map((c) => [c.id, c]));
    const slackConnected = !!(slack.bot_token_in_vault || slack.oauth_credentials);
    const ghReady = !!gh.app_env_present;
    const graphOk =
      demo &&
      demo.v2 === true &&
      (demo.graph_status === "ok" ||
        demo.graph_status === "healthy" ||
        (typeof demo.graph_nodes === "number" && demo.graph_nodes > 0));
    const teamsPill =
      teams.status === "ready"
        ? "up"
        : teams.status === "configured" || teams.app_id_present
          ? "mid"
          : "mid";
    if (statusEl) {
      const simple = isSimpleMode();
      statusEl.innerHTML = simple
        ? [
            `<span class="pill ${slackConnected ? "up" : "mid"}">${slackConnected ? "Chat connected" : "Connect chat"}</span>`,
            `<span class="pill ${ghReady ? "up" : "mid"}">${ghReady ? "GitHub ready" : "Connect GitHub"}</span>`,
            graphOk
              ? `<span class="pill up">Work map healthy</span>`
              : `<span class="pill mid">Work map warming up</span>`,
          ].join(" ")
        : [
            `<span class="pill ${slack.bot_token_in_vault ? "up" : slack.oauth_credentials ? "mid" : "mid"}">Slack: ${
              slack.bot_token_in_vault
                ? "token in vault"
                : slack.oauth_credentials
                  ? "OAuth ready"
                  : "manual vault"
            }</span>`,
            `<span class="pill ${ghReady ? "up" : "mid"}">GitHub App: ${ghReady ? "ready" : "set slug/id"}</span>`,
            `<span class="pill ${teamsPill}">Teams: ${esc(teams.status || "manual")}</span>`,
            `<span class="pill mid">adapter: ${esc(o.delivery_adapter || "slack")}</span>`,
            graphOk
              ? `<span class="pill up">graph: healthy</span>`
              : demo
                ? `<span class="pill mid">graph: ${esc(demo.graph_status || "n/a")}</span>`
                : "",
          ]
            .filter(Boolean)
            .join(" ");
    }
    if ($("conn-github")) {
      $("conn-github").textContent = gh.note || $("conn-github").textContent;
    }
    // Always show webhook URL when server returns it
    if ($("conn-gh-webhook") && gh.webhook_url) {
      $("conn-gh-webhook").textContent = gh.webhook_url;
    }
    if ($("conn-slack-events") && slack.events_url) {
      $("conn-slack-events").textContent = slack.events_url;
    }
    if ($("conn-slack")) {
      const scopes = Array.isArray(slack.scopes) ? slack.scopes.join(", ") : "";
      $("conn-slack").textContent =
        (slack.note || "Outbound digests via egress vault.") +
        (slack.egress_mode ? ` Mode: ${slack.egress_mode}.` : "") +
        (scopes ? ` Scopes: ${scopes}.` : "");
    }
    if ($("conn-slack-manual")) {
      $("conn-slack-manual").textContent = slack.manual_path
        ? `Manual: ${slack.manual_path}`
        : "";
    }
    if ($("conn-teams-note") && teams.note) {
      $("conn-teams-note").textContent = teams.note;
    }
    if ($("conn-teams-manual")) {
      $("conn-teams-manual").textContent = teams.manual_path
        ? `Manual: ${teams.manual_path}` +
          (teams.messaging_endpoint
            ? ` · Messaging: ${teams.messaging_endpoint}`
            : "")
        : teams.messaging_endpoint
          ? `Messaging endpoint: ${teams.messaging_endpoint}`
          : "";
    }
    if ($("conn-sso-note") && o.sso?.note) {
      $("conn-sso-note").textContent = "Google/SSO: " + o.sso.note;
    }
    // Checklist visual from oauth_status + soft graph probe
    const mark = (id, ok, text) => {
      const el = $(id);
      if (!el) return;
      el.textContent = (ok ? "✓ " : "○ ") + text;
      el.style.color = ok ? "var(--up, #0a7)" : "";
      el.style.fontWeight = ok ? "600" : "";
    };
    const slackStep = byId.slack_connect;
    mark(
      "conn-step-slack",
      !!(slack.bot_token_in_vault || slackStep?.done),
      slack.bot_token_in_vault
        ? "Connect Slack — bot token in vault (restart egress once after first connect)"
        : slack.oauth_credentials
          ? "Connect Slack — OAuth ready (button opens Slack install)"
          : "Connect Slack — set SLACK_CLIENT_ID/SECRET or paste vault token"
    );
    const eventsStep = byId.slack_events_url;
    mark(
      "conn-step-events",
      false,
      eventsStep?.label ||
        "Paste Slack Events Request URL (Event Subscriptions) — copy from this page"
    );
    mark(
      "conn-step-channel",
      false,
      byId.slack_bot_channel?.label ||
        "Invite bot to team channel (needed for channel claims; digests still DM)"
    );
    mark(
      "conn-step-gh",
      ghReady || !!byId.github_install?.done,
      ghReady
        ? "Install GitHub App — ready (button opens App install); copy webhook URL"
        : "Install GitHub App — set GITHUB_APP_SLUG / ID or wire webhooks manually"
    );
    mark(
      "conn-step-map",
      false,
      "Map pod under Team (Slack user ids) — bulk import available"
    );
    mark(
      "conn-step-graph",
      !!graphOk,
      graphOk
        ? `Graph healthy (${demo.graph_nodes || 0} nodes) — open Cockpit / digests`
        : "Graph + digests — needs healthy V2 + bridge after GitHub install"
    );
    // Soft post-install if vault already has token
    if (slack.bot_token_in_vault && $("conn-post-install")?.classList.contains("hidden")) {
      // don't force; only when returning from OAuth (boot handles that)
    }
  } catch (e) {
    if (statusEl) {
      statusEl.innerHTML = `<span class="pill mid">install status: ${esc(e.message || "n/a")}</span>`;
    }
  }
}

/** Boot: land on Connections after OAuth return (?view=connections / ?connected=slack). */
function handleConnectReturn() {
  try {
    const params = new URLSearchParams(window.location.search || "");
    const hash = (window.location.hash || "").replace(/^#/, "");
    const viewParam = params.get("view") || (hash.startsWith("view=") ? hash.slice(5) : null);
    const connected = params.get("connected");
    const modeParam = params.get("mode");
    if (modeParam === "simple" || modeParam === "technical") {
      setUxMode(modeParam, { rerender: false });
    }
    const known = ["cockpit", "today", "status", "team", "graph", "connections", "insights", "settings", "lab"];
    if (known.includes(viewParam) || connected === "slack" || connected === "github") {
      showView(known.includes(viewParam) ? viewParam : "connections");
      refreshConnectors().then(() => {
        if (connected === "slack") {
          showPostInstallCallout("slack");
        } else if (connected === "github") {
          showPostInstallCallout("github");
        }
      });
      // Clean query so refresh doesn't re-flash, keep path usable
      if (window.history?.replaceState) {
        const url = new URL(window.location.href);
        url.searchParams.delete("connected");
        // keep view=connections so shareable
        window.history.replaceState({}, "", url.pathname + url.search + url.hash);
      }
      return true;
    }
  } catch (_) {}
  return false;
}

async function saveTomorrowFocus(clear) {
  const tenant = syncTenantFields(activeTenant());
  const noteEl = $("ck-tomorrow-note");
  try {
    let items = [];
    if (!clear) {
      const uniq = window.__ckTomorrowUniq || [];
      items = uniq.slice(0, 12).map((t) => ({
        kind: t.kind,
        text: t.text,
        why: t.why,
      }));
    }
    await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/tomorrow_focus`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        items,
        note: clear ? "Cleared" : "Pinned from cockpit",
      }),
    });
    if (noteEl) {
      noteEl.textContent = clear
        ? "Pins cleared."
        : `Pinned ${items.length} item(s) for tenant ${tenant}.`;
    }
    await refreshCockpit();
  } catch (e) {
    if (noteEl) noteEl.textContent = "Save failed: " + (e.message || e);
  }
}

async function reloadRoles() {
  const tenant = syncTenantFields(activeTenant());
  const msg = $("roles-msg");
  try {
    const r = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/roles`);
    const champs = r.roles?.champions || [];
    if ($("roles-champions")) $("roles-champions").value = champs.join(", ");
    if (msg) {
      msg.textContent = `default_role=${r.roles?.default_role || "champion"} · champions=${champs.length}`;
    }
  } catch (e) {
    if (msg) msg.textContent = "Load failed: " + (e.message || e);
  }
}

async function saveRoles() {
  const tenant = syncTenantFields(activeTenant());
  const msg = $("roles-msg");
  const raw = ($("roles-champions")?.value || "").trim();
  const champions = raw
    ? raw
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean)
    : [];
  try {
    await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/roles`, {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        champions,
        default_role: champions.length ? "member" : "champion",
      }),
    });
    if (msg) msg.textContent = `Saved ${champions.length} champion(s).`;
  } catch (e) {
    if (msg) msg.textContent = "Save failed: " + (e.message || e);
  }
}

function esc(s) {
  return String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function refreshTeamDigestsToday() {
  const el = $("today-team-digests");
  if (!el) return;
  const tenant = syncTenantFields(activeTenant());
  try {
    const team = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team`);
    const members = team.members || [];
    if (!members.length) {
      el.innerHTML = `<p class="muted">No team members yet — open <strong>Team</strong> and map ≥2 people.</p>`;
      return;
    }
    el.innerHTML =
      `<div class="meta-row" style="margin-bottom:0.5rem;">` +
      `<span class="pill ${team.multi_person_ready ? "up" : "mid"}">${
        isSimpleMode()
          ? team.multi_person_ready
            ? "Team of 2+ ready"
            : "Add at least 2 people"
          : team.multi_person_ready
            ? "multi-person: ready"
            : "need ≥2"
      }</span>` +
      `<span class="muted small" style="margin-left:0.5rem;">Click a person → My update</span>` +
      `</div><ul class="item-list">` +
      members
        .map((m) => {
          const d = m.last_digest;
          let dig;
          if (!d) {
            dig = `<span class="muted">no digest yet</span>`;
          } else {
            const content =
              d.has_content === true
                ? "has items"
                : d.empty_placeholder
                  ? "empty window"
                  : d.approx_item_count > 0
                    ? `${d.approx_item_count} item(s)`
                    : "draft";
            dig = `<strong>${esc(d.status_label || d.status)}</strong> · ${content} · ${d.dm_sent ? "DM sent" : "no DM"} · <span class="muted small">${scrubTextHtml((d.preview || "").slice(0, 80))}</span>`;
          }
          const did = d?.draft_id || "";
          const lid = d?.ledger_id || "";
          return `<li><button type="button" class="ghost graph-filter-btn dig-open" data-draft="${esc(did)}" data-ledger="${esc(lid)}" data-name="${esc(m.display_name || m.subject_id)}"><strong>${esc(m.display_name || m.subject_id)}</strong></button> — ${dig}</li>`;
        })
        .join("") +
      `</ul>`;
    el.querySelectorAll(".dig-open").forEach((btn) => {
      btn.addEventListener("click", async () => {
        const did = btn.getAttribute("data-draft");
        const lid = btn.getAttribute("data-ledger");
        if (!did) {
          alert("No draft yet — Team → Compile all digests first");
          return;
        }
        const ok = await openDraftById(tenant, did, lid);
        if (ok) showView("status");
        else alert("Could not open draft");
      });
    });
  } catch (e) {
    el.innerHTML = `<p class="muted">Team digests unavailable: ${esc(e.message || e)}</p>`;
  }
}

async function refreshPulse() {
  const tenant =
    $("team-tenant")?.value?.trim() ||
    $("tenant")?.value?.trim() ||
    "ten_github";
  const el = $("today-conflicts");
  try {
    await refreshTeamDigestsToday();
    const pulse = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/pulse?refresh=true`
    );
    const cards = pulse.conflicts?.cards || [];
    const count = pulse.conflicts?.count ?? cards.length;
    const demoCount = pulse.conflicts?.demo_count ?? 0;
    const multi = pulse.team?.multi_person_ready;
    if (el) {
      if (!count) {
        const simple = isSimpleMode();
        const demoNote =
          demoCount > 0
            ? simple
              ? ` Example stories stay hidden.`
              : ` <span class="muted small">(${demoCount} example card(s) hidden)</span>`
            : "";
        el.innerHTML = simple
          ? `<p class="muted">No open friction right now.${demoNote}</p>`
          : `<p class="muted">No open live conflicts. Multi-person ready: <strong>${multi ? "yes" : "no"}</strong> (${pulse.team?.unique_slack_users ?? pulse.team?.slack_mapped ?? 0} unique Slack).${demoNote}</p>`;
      } else {
        el.innerHTML =
          `<div class="meta-row"><span class="pill ${count ? "down" : "mid"}">${count} live conflict(s)</span>` +
          (demoCount
            ? `<span class="pill mid">${demoCount} demo seed</span>`
            : "") +
          `<span class="pill ${multi ? "up" : "mid"}">multi-person: ${multi ? "ready" : "need ≥2 maps"}</span></div>` +
          `<ul class="item-list">` +
          cards
            .slice(0, 12)
            .map(
              (c) =>
                `<li><strong>[${esc(c.severity || c.kind)}]</strong> ${scrubTextHtml(c.summary)} <span class="muted small">${esc(c.kind)}</span></li>`
            )
            .join("") +
          `</ul>`;
      }
    }
    const intentUl = $("team-intents");
    if (intentUl) {
      const sample = pulse.intents?.sample || [];
      const demoIntents = pulse.intents?.demo_count ?? 0;
      if (!sample.length) {
        intentUl.innerHTML = `<li class="muted">No live intent nodes yet${demoIntents ? ` (${demoIntents} demo seed hidden)` : ""} — project PRs/issues with titles/labels, or Load intent demo for UI proof.</li>`;
      } else {
        intentUl.innerHTML = sample
          .slice(0, 20)
          .map((n) => {
            const ty =
              n.properties?.intent_type ||
              n.intent_type ||
              "?";
            return `<li><strong>[${esc(ty)}]</strong> ${prettyMaybe(n.display_name) || prettyRef(n.node_id)}</li>`;
          })
          .join("");
      }
    }
  } catch (e) {
    if (el) {
      el.innerHTML = `<p class="muted">Pulse unavailable: ${esc(e.message || e)}</p>`;
    }
  }
}

function digestCell(m) {
  const d = m.last_digest;
  if (!d) {
    return `<span class="muted small">no digest yet</span>`;
  }
  const dm = d.dm_sent ? "DM sent" : "no DM";
  const st = d.status_label || d.status || "?";
  const content =
    d.has_content === true
      ? "items"
      : d.empty_placeholder
        ? "empty"
        : "draft";
  const when = d.updated_at ? fmtIst(d.updated_at) : "";
  return `<span class="pill ${d.has_content ? "up" : "mid"}">${esc(st)} · ${content}</span> <span class="muted small">${esc(dm)}${when ? " · " + esc(when) : ""}</span>`;
}

async function refreshTeam() {
  const tenant = $("team-tenant")?.value?.trim() || "ten_github";
  const body = $("team-body");
  const ready = $("team-ready");
  const simple = isSimpleMode();
  try {
    const team = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team`);
    if (ready) {
      const withDigest = (team.members || []).filter((m) => m.last_digest).length;
      ready.innerHTML = simple
        ? [
            `<span class="pill ${team.multi_person_ready ? "up" : "mid"}">${team.multi_person_ready ? "Team of 2+ ready" : "Add at least 2 people"}</span>`,
            `<span class="pill mid">${team.slack_mapped_count ?? 0} connected to chat</span>`,
            `<span class="pill mid">${withDigest} with a status update</span>`,
          ].join("")
        : [
            `<span class="pill ${team.multi_person_ready ? "up" : "mid"}">multi-person: ${team.multi_person_ready ? "ready" : "need ≥2 Slack maps"}</span>`,
            `<span class="pill mid">${team.slack_mapped_count ?? 0} mapped / ${team.person_count ?? 0} members</span>`,
            `<span class="pill mid">${withDigest} with digests</span>`,
          ].join("");
    }
    // Simple: visual people grid above/instead of dense table
    let simpleHost = $("team-simple-grid");
    if (!simpleHost && body?.closest(".card")) {
      simpleHost = document.createElement("div");
      simpleHost.id = "team-simple-grid";
      simpleHost.className = "viz-people simple-panel";
      body.closest(".card").insertBefore(simpleHost, body.closest("table") || body.parentElement);
    }
    if (simpleHost) {
      const members = team.members || [];
      if (simple) {
        simpleHost.classList.remove("hidden");
        if (body?.closest("table")) body.closest("table").classList.add("hidden");
        simpleHost.innerHTML = members.length
          ? members
              .map((m) => {
                const name = m.display_name || "";
                const letter = (name || "?").slice(0, 1).toUpperCase();
                return `<div class="viz-person">
                  <span class="ux-avatar">${esc(letter)}</span>
                  <span class="viz-person-name">${displayNameOrEye(name, m.subject_id)}</span>
                  <span class="pill ${m.slack_mapped ? "up" : "mid"}">${m.slack_mapped ? "Chat linked" : "No chat yet"}</span>
                  <span class="pill ${m.last_digest?.has_content ? "up" : "mid"}">${esc(plainDigestStatus(m.last_digest))}</span>
                </div>`;
              })
              .join("")
          : `<div class="viz-empty">No people yet — add someone below.</div>`;
      } else {
        simpleHost.classList.add("hidden");
        if (body?.closest("table")) body.closest("table").classList.remove("hidden");
      }
    }
    if (body) {
      const members = team.members || [];
      if (!members.length) {
        body.innerHTML = `<tr><td colspan="5" class="muted">No members yet — add two humans below.</td></tr>`;
      } else {
        body.innerHTML = members
          .map((m) => {
            const aliases = (Array.isArray(m.provider_aliases) ? m.provider_aliases : [])
              .map((a) => prettyMaybe(a))
              .join(" ");
            const sub = simple
              ? displayNameOrEye(m.display_name, m.subject_id)
              : `${prettyMaybe(m.subject_id)}${aliases ? ` <span class="muted">${aliases}</span>` : ""}`;
            return `<tr>
              <td>${displayNameOrEye(m.display_name, m.subject_id)}</td>
              <td>${sub}</td>
              <td>${m.slack_user_id ? prettyRef(m.slack_user_id) : "—"}</td>
              <td>${m.slack_mapped ? "✓" : "○"}</td>
              <td>${digestCell(m)}</td>
            </tr>`;
          })
          .join("");
      }
    }
    await refreshPulse();
  } catch (e) {
    if (body) {
      body.innerHTML = `<tr><td colspan="5" class="muted">Team load failed: ${esc(e.message || e)}</td></tr>`;
    }
  }
}

async function compileTeamDigests() {
  const tenant = $("team-tenant")?.value?.trim() || "ten_github";
  const msg = $("team-compile-msg");
  const btn = $("btn-team-compile");
  if (btn) {
    btn.disabled = true;
    btn.textContent = "Writing…";
  }
  try {
    const out = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team/compile`, {
      method: "POST",
      body: JSON.stringify({ force_notify: false, allow_notify: true }),
    });
    if (msg) {
      const lines = (out.results || [])
        .map((r) => {
          if (r.ok === false) return `${r.display_name || r.twin_id}: ERR ${r.error || ""}`;
          const kinds = (r.item_kinds || []).slice(0, 4).join(",") || "—";
          const why = r.empty ? ` empty(${r.empty_reason || "?"})` : "";
          const sum = (r.item_summaries || [])[0]
            ? ` · ${(r.item_summaries[0] || "").slice(0, 48)}`
            : "";
          return `${r.display_name || r.twin_id}: items=${r.item_count ?? 0} [${kinds}] dm=${r.dm_sent ? "yes" : "no"}${r.suppressed ? " (" + r.suppressed + ")" : ""}${why}${sum}`;
        })
        .join(" · ");
      const withItems = out.with_items != null ? `, with_items ${out.with_items}` : "";
      msg.textContent = `Compiled ${out.compiled ?? 0}${withItems}, DMs ${out.dms_sent ?? 0}, window ${out.status_window_secs ?? "?"}s. ${lines}`;
    }
    await refreshTeam();
  } catch (e) {
    if (msg) msg.textContent = "Compile all failed: " + (e.message || e);
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "Write all updates";
    }
  }
}

async function addTeamMember() {
  const tenant = $("team-tenant")?.value?.trim() || "ten_github";
  const subject = $("tm-subject")?.value?.trim();
  const slack = $("tm-slack")?.value?.trim();
  const name = $("tm-name")?.value?.trim();
  const channel = $("tm-channel")?.value?.trim();
  const aliasesRaw = $("tm-aliases")?.value?.trim() || "";
  const msg = $("team-add-msg");
  if (!subject || !slack) {
    if (msg) msg.textContent = "subject_id and slack_user_id are required";
    return;
  }
  const provider_aliases = aliasesRaw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
  try {
    await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team/members`, {
      method: "POST",
      body: JSON.stringify({
        subject_id: subject,
        display_name: name || subject,
        slack_user_id: slack,
        channel_id: channel || undefined,
        provider_aliases,
        skip_shadow: true,
        enabled: true,
      }),
    });
    if (msg) msg.textContent = "Saved. Bridge will pick up map on next poll.";
    $("tm-subject").value = "";
    $("tm-aliases").value = "";
    await refreshTeam();
  } catch (e) {
    if (msg) msg.textContent = "Save failed: " + (e.message || e);
  }
}

async function refreshMetrics() {
  try {
    const m = await jfetch("/metrics");
    if ($("met-dms")) $("met-dms").textContent = String(m.twin_dms_sent_total ?? m.twin_drafts_sent_total ?? "—");
    if ($("met-policy")) $("met-policy").textContent = String(m.notify_policy ?? "v1_change_only_daily_cap");
    if ($("met-suppress")) $("met-suppress").textContent = String(m.twin_dms_suppressed_total ?? "—");
    if ($("met-veto")) {
      const r = m.twin_veto_rate;
      $("met-veto").textContent =
        r == null ? "—" : `${Math.round(Number(r) * 1000) / 10}%`;
    }
    if ($("met-empty")) $("met-empty").textContent = String(m.twin_empty_windows_total ?? "—");
    if ($("met-conflicts")) $("met-conflicts").textContent = String(m.twin_conflict_hits_total ?? "—");
  } catch {
    /* ignore */
  }
}

/* ═══════════════════════════════════════════════════════════════
   Graph live map — hierarchical layout (people → work → intents)
   Avoids commit hairball circle; recent-commits-only by default.
   ═══════════════════════════════════════════════════════════════ */
const graphState = {
  raw: null,
  nodes: [], // { id, type, label, x, y, vx, vy, r, meta, pinSoft, visual }
  edges: [], // { id, type, from, to }
  filters: {}, // type -> bool
  selected: null,
  liveTimer: null,
  anim: null,
  drag: null,
  pan: { x: 0, y: 0 },
  scale: 1,
  panning: null,
  lastFetch: 0,
  types: [],
  frame: 0,
  fitTimer: null,
  storyTried: false,
  settled: false,
};

const GRAPH_TYPE_ORDER = [
  "Person",
  "PullRequest",
  "Issue",
  "Ticket",
  "Intent",
  "Repo",
  "Team",
  "Commit",
  "Channel",
];

/** Core story nodes — always preferred in layout/fit */
const GRAPH_HUB_TYPES = new Set([
  "Person",
  "PullRequest",
  "Issue",
  "Ticket",
  "Intent",
  "Repo",
  "Team",
]);

/** Max commits drawn when "Recent commits" is on (stops the circle hairball). */
const COMMIT_VISUAL_MAX = 14;

function graphTenant() {
  return (
    $("graph-tenant")?.value?.trim() ||
    $("team-tenant")?.value?.trim() ||
    "ten_github"
  );
}

function stopGraphLive() {
  if (graphState.liveTimer) {
    clearInterval(graphState.liveTimer);
    graphState.liveTimer = null;
  }
  if (graphState.anim) {
    cancelAnimationFrame(graphState.anim);
    graphState.anim = null;
  }
}

function startGraphView() {
  ensureGraphCanvas();
  refreshGraph(true);
  stopGraphLive();
  if ($("graph-live")?.checked !== false) {
    graphState.liveTimer = setInterval(() => {
      if ($("graph-live")?.checked) refreshGraph(false);
    }, 5000);
  }
  if (!graphState.anim) {
    const tick = () => {
      stepForce();
      drawGraph();
      graphState.anim = requestAnimationFrame(tick);
    };
    graphState.anim = requestAnimationFrame(tick);
  }
}

function ensureGraphCanvas() {
  const canvas = $("graph-canvas");
  if (!canvas || canvas._graphBound) return;
  canvas._graphBound = true;
  const resize = () => {
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const h = Math.max(640, rect.height || 680);
    canvas.width = Math.max(480, Math.floor(rect.width * dpr));
    canvas.height = Math.floor(h * dpr);
    canvas.style.height = h + "px";
    drawGraph();
  };
  window.addEventListener("resize", resize);
  resize();

  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    // Zoom toward cursor (not just scale about origin)
    const rect = canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const mx = (e.clientX - rect.left) * dpr;
    const my = (e.clientY - rect.top) * dpr;
    const worldX = (mx - graphState.pan.x) / graphState.scale;
    const worldY = (my - graphState.pan.y) / graphState.scale;
    const factor = e.deltaY > 0 ? 0.9 : 1.12;
    const next = Math.min(2.8, Math.max(0.45, graphState.scale * factor));
    graphState.scale = next;
    graphState.pan.x = mx - worldX * next;
    graphState.pan.y = my - worldY * next;
    drawGraph();
  }, { passive: false });

  canvas.addEventListener("pointerdown", (e) => {
    const pt = canvasPoint(canvas, e);
    const hit = hitNode(pt.x, pt.y);
    if (hit) {
      graphState.selected = hit.id;
      graphState.drag = { id: hit.id, ox: hit.x, oy: hit.y };
      hit.fx = hit.x;
      hit.fy = hit.y;
      canvas.setPointerCapture(e.pointerId);
      renderGraphDetail(hit);
      drawGraph();
      return;
    }
    graphState.panning = { x: e.clientX, y: e.clientY, px: graphState.pan.x, py: graphState.pan.y };
    canvas.setPointerCapture(e.pointerId);
  });
  canvas.addEventListener("pointermove", (e) => {
    if (graphState.drag) {
      const pt = canvasPoint(canvas, e);
      const n = graphState.nodes.find((x) => x.id === graphState.drag.id);
      if (n) {
        n.x = pt.x;
        n.y = pt.y;
        n.fx = pt.x;
        n.fy = pt.y;
        n.vx = 0;
        n.vy = 0;
      }
      return;
    }
    if (graphState.panning) {
      graphState.pan.x = graphState.panning.px + (e.clientX - graphState.panning.x);
      graphState.pan.y = graphState.panning.py + (e.clientY - graphState.panning.y);
    }
  });
  canvas.addEventListener("pointerup", () => {
    if (graphState.drag) {
      const n = graphState.nodes.find((x) => x.id === graphState.drag.id);
      if (n) {
        n.fx = null;
        n.fy = null;
      }
    }
    graphState.drag = null;
    graphState.panning = null;
  });
  canvas.addEventListener("pointercancel", () => {
    graphState.drag = null;
    graphState.panning = null;
  });
}

function canvasPoint(canvas, e) {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  const cx = (e.clientX - rect.left) * dpr;
  const cy = (e.clientY - rect.top) * dpr;
  return {
    x: (cx - graphState.pan.x) / graphState.scale,
    y: (cy - graphState.pan.y) / graphState.scale,
  };
}

function hitNode(x, y) {
  for (let i = graphState.nodes.length - 1; i >= 0; i--) {
    const n = graphState.nodes[i];
    if (!nodeVisible(n)) continue;
    const dx = n.x - x;
    const dy = n.y - y;
    if (dx * dx + dy * dy <= (n.r + 6) * (n.r + 6)) return n;
  }
  return null;
}

function typeVisible(t) {
  if (graphState.filters[t] === false) return false;
  return true;
}

function nodeVisible(n) {
  if (!n || !typeVisible(n.type)) return false;
  if (n.visual === false) return false;
  return true;
}

function normalizeType(t) {
  if (!t) return "Other";
  if (t === "pull_request") return "PullRequest";
  if (t === "issue" || t === "ticket") return "Issue";
  return t;
}

function nodeRadius(type) {
  switch (type) {
    case "Person": return 20;
    case "PullRequest": return 14;
    case "Issue":
    case "Ticket": return 13;
    case "Intent": return 13;
    case "Repo": return 18;
    case "Commit": return 7;
    default: return 10;
  }
}

function edgeTimeMs(e) {
  const vf = e.valid_from || e.meta?.valid_from || "";
  const t = Date.parse(vf);
  return Number.isFinite(t) ? t : 0;
}

async function refreshGraph(forceLayout) {
  const tenant = graphTenant();
  const statsEl = $("graph-stats");
  try {
    if (statsEl && forceLayout) {
      statsEl.innerHTML = `<span class="pill mid">loading map…</span>`;
    }
    const includeDemo = $("graph-hide-demo")?.checked === false;
    const showUnapproved = $("graph-show-unapproved")?.checked === true;
    const data = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/graph?node_limit=600&edge_limit=1500&include_demo=${includeDemo ? "true" : "false"}&show_unapproved=${showUnapproved ? "true" : "false"}`
    );
    graphState.raw = data;
    graphState.lastFetch = Date.now();
    mergeGraphData(data, forceLayout);
    renderGraphChrome(data);
    // Auto-enrich once if the map is commit-only (no intents/PRs for the story)
    maybeAutoEnrichGraph(data).catch(() => {});
    drawGraph();
  } catch (e) {
    if (statsEl) {
      statsEl.innerHTML = `<span class="pill down">graph load failed: ${esc(e.message || e)}</span>`;
    }
  }
}

async function maybeAutoEnrichGraph(_data) {
  // Disabled: never mutate graph mid-sales call. Use "Enrich story" explicitly.
  return;
}

function mergeGraphData(data, forceLayout) {
  const prev = new Map(graphState.nodes.map((n) => [n.id, n]));
  const types = new Set();
  const edgeDeg = new Map();
  for (const e of data.edges || []) {
    edgeDeg.set(e.from, (edgeDeg.get(e.from) || 0) + 1);
    edgeDeg.set(e.to, (edgeDeg.get(e.to) || 0) + 1);
  }
  const hideDemo =
    $("graph-hide-demo")?.checked !== false; /* default hide alice/bob seed */
  const bestByLabel = new Map();
  for (const n of data.nodes || []) {
    if (normalizeType(n.type) !== "Person") continue;
    if (n.duplicate_person) continue;
    const lab = String(n.label || n.id || "").toLowerCase();
    if (hideDemo && (lab === "alice" || lab === "bob" || lab.includes("demo_alice") || lab.includes("demo_bob"))) continue;
    if (hideDemo && String(n.id || "").includes("gu_demo_")) continue;
    const prevN = bestByLabel.get(lab);
    if (!prevN) {
      bestByLabel.set(lab, n);
      continue;
    }
    const dNew = edgeDeg.get(n.id) || 0;
    const dOld = edgeDeg.get(prevN.id) || 0;
    const score = (node, deg) => {
      let s = deg * 10;
      if (!node.from_team_map) s += 3;
      if (/^\d+$/.test(String(node.resource_id || ""))) s += 2;
      if (!String(node.id || "").includes("gu_seed_")) s += 1;
      return s;
    };
    if (score(n, dNew) > score(prevN, dOld)) bestByLabel.set(lab, n);
  }
  const keepPersonIds = new Set([...bestByLabel.values()].map((n) => n.id));
  const aliasTo = new Map();
  for (const n of data.nodes || []) {
    if (normalizeType(n.type) !== "Person") continue;
    const lab = String(n.label || n.id || "").toLowerCase();
    const keep = bestByLabel.get(lab);
    if (keep && keep.id !== n.id) aliasTo.set(n.id, keep.id);
  }
  let edgesIn = (data.edges || []).map((e) => ({
    ...e,
    from: aliasTo.get(e.from) || e.from,
    to: aliasTo.get(e.to) || e.to,
  }));
  // Drop self-loops after collapse
  edgesIn = edgesIn.filter((e) => e.from !== e.to);

  const rawNodes = (data.nodes || []).filter((n) => {
    const t = normalizeType(n.type);
    if (hideDemo && (t === "Person" || String(n.id || "").includes("gu_demo_"))) {
      if (String(n.id || "").includes("gu_demo_")) return false;
      if (String(n.label || "").toLowerCase() === "alice" || String(n.label || "").toLowerCase() === "bob") return false;
    }
    if (hideDemo && String(n.id || "").includes("demo-repo")) return false;
    if (hideDemo && n.seed === "intent_demo") return false;
    if (t === "Person") return keepPersonIds.has(n.id);
    return true;
  });

  // Rank commits by newest linked edge — only draw top N when recent-only is on
  const commitScore = new Map();
  for (const e of edgesIn) {
    const ts = edgeTimeMs(e);
    for (const end of [e.from, e.to]) {
      if (String(end).startsWith("commit:") || rawNodes.find((n) => n.id === end && normalizeType(n.type) === "Commit")) {
        commitScore.set(end, Math.max(commitScore.get(end) || 0, ts));
      }
    }
  }
  const commitIds = rawNodes
    .filter((n) => normalizeType(n.type) === "Commit")
    .map((n) => n.id)
    .sort((a, b) => (commitScore.get(b) || 0) - (commitScore.get(a) || 0));
  const recentOnly = $("graph-recent-commits")?.checked !== false;
  const keepCommit = new Set(
    recentOnly ? commitIds.slice(0, COMMIT_VISUAL_MAX) : commitIds
  );

  const nodes = rawNodes.map((n, i) => {
    const type = normalizeType(n.type);
    types.add(type);
    const old = prev.get(n.id);
    const r = nodeRadius(type);
    const visual =
      type !== "Commit" ? true : keepCommit.has(n.id);
    if (old && !forceLayout) {
      return {
        ...old,
        type,
        label: displayLabel(n, type),
        r,
        meta: n,
        visual,
      };
    }
    return {
      id: n.id,
      type,
      label: displayLabel(n, type),
      x: old?.x ?? 0,
      y: old?.y ?? 0,
      vx: 0,
      vy: 0,
      r,
      fx: null,
      fy: null,
      pinSoft: null,
      meta: n,
      decision: n.decision || n.intent_type || "",
      visual,
      _seedI: i,
    };
  });

  graphState.nodes = nodes;
  graphState.edges = edgesIn.map((e) => ({
    id: e.id,
    type: e.type || "RELATED",
    from: e.from,
    to: e.to,
    meta: e,
    valid_from: e.valid_from,
  }));

  if (forceLayout || !prev.size) {
    applyHierarchicalSeed(graphState.nodes, graphState.edges);
    graphState.settled = false;
    graphState.frame = 0;
  }

  for (const t of types) {
    if (graphState.filters[t] === undefined) {
      // Default: everything on (commits already capped visually)
      graphState.filters[t] = true;
    }
  }
  graphState.types = Array.from(types).sort((a, b) => {
    const ia = GRAPH_TYPE_ORDER.indexOf(a);
    const ib = GRAPH_TYPE_ORDER.indexOf(b);
    return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.localeCompare(b);
  });
  renderGraphFilters();
  if (forceLayout) {
    // Fit hubs after a short settle — avoids aggressive zoom-out on cold start
    scheduleFitAfterSettle();
  }
}

function displayLabel(n, type) {
  if (type === "Commit") {
    const msg = (n.title || n.message || "").toString().trim();
    const sha = (n.label || "").toString();
    if (msg && msg !== sha && !isOpaqueToken(msg)) return msg.slice(0, 48);
    return "Commit";
  }
  if (type === "Intent") {
    const lab = n.intent_type || n.label || "";
    if (lab && !isOpaqueToken(lab)) return lab;
    return "Intent";
  }
  if (type === "PullRequest") {
    const t = (n.title || n.label || "").toString();
    if (t && !isOpaqueToken(t)) return t.slice(0, 48);
    return "Pull request";
  }
  const lab = (n.label || "").toString();
  if (lab && !isOpaqueToken(lab)) return lab;
  if (type === "Person") return "Person";
  if (type === "Repo") return "Repo";
  return type || "Item";
}

/** Place people on top row, repo center, work mid, intents near work, commits fan under author. */
function applyHierarchicalSeed(nodes, edges) {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const people = nodes.filter((n) => n.type === "Person" && n.visual !== false);
  const repos = nodes.filter((n) => n.type === "Repo" && n.visual !== false);
  const work = nodes.filter(
    (n) =>
      (n.type === "PullRequest" || n.type === "Issue" || n.type === "Ticket") &&
      n.visual !== false
  );
  const intents = nodes.filter((n) => n.type === "Intent" && n.visual !== false);
  const digests = nodes.filter((n) => n.type === "StatusDigest" && n.visual !== false);
  const commits = nodes.filter((n) => n.type === "Commit" && n.visual !== false);
  const other = nodes.filter(
    (n) =>
      n.visual !== false &&
      ![
        "Person",
        "Repo",
        "PullRequest",
        "Issue",
        "Ticket",
        "Intent",
        "Commit",
        "StatusDigest",
      ].includes(n.type)
  );

  // Author map for commits via AUTHORED
  const authorOf = new Map();
  for (const e of edges) {
    if (e.type === "AUTHORED") {
      const from = byId.get(e.from);
      const to = byId.get(e.to);
      if (from?.type === "Person" && to?.type === "Commit") authorOf.set(to.id, from.id);
      if (to?.type === "Person" && from?.type === "Commit") authorOf.set(from.id, to.id);
    }
  }
  // Intent about work via ABOUT/CLAIMS
  const aboutOf = new Map();
  for (const e of edges) {
    if (e.type === "ABOUT" || e.type === "CLAIMS") {
      const a = byId.get(e.from);
      const b = byId.get(e.to);
      if (a?.type === "Intent" && b && (b.type === "PullRequest" || b.type === "Issue")) {
        aboutOf.set(a.id, b.id);
      }
      if (b?.type === "Intent" && a && (a.type === "PullRequest" || a.type === "Issue")) {
        aboutOf.set(b.id, a.id);
      }
    }
  }

  const spacing = 180;
  people.forEach((p, i) => {
    const x = (i - (people.length - 1) / 2) * spacing;
    const y = -180;
    p.x = x;
    p.y = y;
    p.pinSoft = { x, y, k: 0.045 };
  });

  repos.forEach((r, i) => {
    const x = (i - (repos.length - 1) / 2) * 220;
    const y = 20;
    r.x = x;
    r.y = y;
    r.pinSoft = { x, y, k: 0.05 };
  });

  work.forEach((w, i) => {
    // Place under the people band, left-to-right
    const x = (i - (work.length - 1) / 2) * 160;
    const y = -40;
    w.x = x;
    w.y = y;
    w.pinSoft = { x, y, k: 0.03 };
  });

  intents.forEach((it, i) => {
    const about = aboutOf.get(it.id);
    const anchor = about ? byId.get(about) : null;
    const x = (anchor?.x ?? (i - (intents.length - 1) / 2) * 140) + (i % 2 === 0 ? -50 : 50);
    const y = (anchor?.y ?? -40) - 90;
    it.x = x;
    it.y = y;
    it.pinSoft = { x, y, k: 0.04 };
  });

  digests.forEach((dg, i) => {
    const person = people[i % Math.max(1, people.length)];
    const x = (person?.x ?? (i - (digests.length - 1) / 2) * 100) + 70;
    const y = (person?.y ?? -180) - 70;
    dg.x = x;
    dg.y = y;
    dg.pinSoft = { x, y, k: 0.05 };
  });

  // Commits: fan under each author (or under first person)
  const byAuthor = new Map();
  for (const c of commits) {
    const a = authorOf.get(c.id) || people[0]?.id || "";
    if (!byAuthor.has(a)) byAuthor.set(a, []);
    byAuthor.get(a).push(c);
  }
  for (const [aid, list] of byAuthor) {
    const person = byId.get(aid) || people[0];
    const px = person?.x ?? 0;
    const py = person?.y ?? -180;
    list.forEach((c, i) => {
      const col = i % 5;
      const row = Math.floor(i / 5);
      const x = px + (col - 2) * 36;
      const y = py + 90 + row * 32;
      c.x = x;
      c.y = y;
      c.pinSoft = { x, y, k: 0.025 };
    });
  }

  other.forEach((n, i) => {
    const x = (i - (other.length - 1) / 2) * 100;
    const y = 140;
    n.x = x;
    n.y = y;
    n.pinSoft = { x, y, k: 0.02 };
  });
}

function scheduleFitAfterSettle() {
  if (graphState.fitTimer) clearTimeout(graphState.fitTimer);
  // Immediate soft fit so user sees something centered
  fitGraph({ hubsOnly: true, maxScale: 1.15, minScale: 0.7 });
  graphState.fitTimer = setTimeout(() => {
    fitGraph({ hubsOnly: true, maxScale: 1.25, minScale: 0.65 });
    graphState.settled = true;
    drawGraph();
  }, 700);
}

function renderGraphFilters() {
  const el = $("graph-filters");
  if (!el) return;
  el.innerHTML = graphState.types
    .map((t) => {
      const on = graphState.filters[t] !== false;
      const total = graphState.nodes.filter((n) => n.type === t).length;
      const vis = graphState.nodes.filter((n) => n.type === t && n.visual !== false).length;
      const countLabel = total === vis ? String(total) : `${vis}/${total}`;
      return `<button type="button" class="ghost graph-filter-btn ${on ? "" : "off"}" data-type="${esc(t)}">${esc(t)} (${countLabel})</button>`;
    })
    .join("");
  el.querySelectorAll("[data-type]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const t = btn.getAttribute("data-type");
      graphState.filters[t] = graphState.filters[t] === false;
      renderGraphFilters();
      scheduleFitAfterSettle();
      drawGraph();
    });
  });
}

function renderGraphChrome(data) {
  const statsEl = $("graph-stats");
  const totals = data.totals || {};
  const returned = data.returned || {
    nodes: (data.nodes || []).length,
    edges: (data.edges || []).length,
  };
  const live = $("graph-live")?.checked;
  const v2Up = data.v2_up !== false && data.error !== "v2_unreachable";
  const status = data.status || (v2Up ? "ok" : "v2_down");
  if (statsEl) {
    const simple = isSimpleMode();
    if (simple) {
      const n = returned.nodes ?? graphState.nodes.length;
      const e = returned.edges ?? graphState.edges.length;
      statsEl.innerHTML = [
        live
          ? `<span class="pill up"><span class="graph-live-dot"></span>Live map</span>`
          : `<span class="pill mid">Paused</span>`,
        v2Up
          ? `<span class="pill up">Connected</span>`
          : `<span class="pill down">Map service offline</span>`,
        `<span class="pill mid">${n} items</span>`,
        `<span class="pill mid">${e} links</span>`,
      ].join(" ");
    } else {
      const statusPill =
        status === "v2_down" || !v2Up
          ? `<span class="pill down">V2 down — recovering</span>`
          : status === "empty_or_error" && !(totals.nodes > 0)
            ? `<span class="pill mid">V2 up · map empty (bridge re-projecting)</span>`
            : `<span class="pill up">V2 up</span>`;
      statsEl.innerHTML = [
        live ? `<span class="pill up"><span class="graph-live-dot"></span>live</span>` : `<span class="pill mid">paused</span>`,
        statusPill,
        `<span class="pill mid">nodes ${returned.nodes ?? graphState.nodes.length}/${totals.nodes ?? "—"}</span>`,
        `<span class="pill mid">edges ${returned.edges ?? graphState.edges.length}/${totals.edges ?? "—"}</span>`,
        data.truncated ? `<span class="pill mid">truncated</span>` : "",
        `<span class="pill mid">reader ${esc(data.reader || "—")}</span>`,
        `<span class="pill mid">as_of ${esc(fmtIst(data.as_of))}</span>`,
      ]
        .filter(Boolean)
        .join("");
    }
  }
  // Banner under toolbar for durable ops signal
  let banner = $("graph-banner");
  if (!banner && $("graph-stats")?.parentElement) {
    banner = document.createElement("div");
    banner.id = "graph-banner";
    banner.className = "graph-banner hidden";
    $("graph-stats").parentElement.insertBefore(banner, $("graph-stats").nextSibling);
  }
  if (banner) {
    if (!v2Up || data.error === "v2_unreachable") {
      banner.classList.remove("hidden");
      banner.innerHTML = `<strong>Graph service (V2) is unreachable.</strong> ${esc(
        data.message ||
          "Autoheal restarts unhealthy V2; bridge pauses until /healthz recovers, then re-projects ingested events."
      )}`;
    } else if ((totals.nodes === 0 || totals.nodes == null) && !(data.nodes || []).length) {
      banner.classList.remove("hidden");
      banner.innerHTML = `<strong>Map is empty.</strong> ${esc(
        data.message ||
          "After a redeploy, only durable journals refill the map. New GitHub activity re-projects within ~2 min."
      )} <button type="button" class="ghost" id="btn-banner-enrich">Enrich story</button>`;
      setTimeout(() => {
        const b = $("btn-banner-enrich");
        if (b && !b._bound) {
          b._bound = true;
          b.addEventListener("click", () => enrichGraphStory());
        }
      }, 0);
    } else {
      banner.classList.add("hidden");
      banner.innerHTML = "";
    }
  }
  const legend = $("graph-legend");
  if (legend) {
    legend.innerHTML = isSimpleMode()
      ? [
          `<span><i></i> People</span>`,
          `<span><i class="pr"></i> Reviews / PRs</span>`,
          `<span><i class="issue"></i> Issues</span>`,
          `<span><i class="intent"></i> Focus / goals</span>`,
          `<span><i class="repo"></i> Projects</span>`,
          `<span>Lines show who is connected to what</span>`,
        ].join("")
      : [
          `<span><i></i> Person</span>`,
          `<span><i class="pr"></i> Pull request</span>`,
          `<span><i class="issue"></i> Issue</span>`,
          `<span><i class="intent"></i> Intent</span>`,
          `<span><i class="repo"></i> Repo</span>`,
          `<span>Lines = edges (AUTHORED, CLAIMS, BLOCKS…)</span>`,
        ].join("");
  }
  const people = $("graph-people");
  if (people) {
    const persons = graphState.nodes.filter((n) => n.type === "Person");
    const team = data.team?.members || [];
    if (!persons.length && !team.length) {
      people.innerHTML = `<li class="muted">No people yet — ingest PRs or add Team maps.</li>`;
    } else {
      const rows = persons.map((p) => {
        const mapped = team.find((m) => m.person_node_id === p.id || `person:${m.subject_id}` === p.id);
        return `<li><button type="button" class="ghost graph-filter-btn" data-node="${esc(p.id)}">${esc(p.label)}</button> ${mapped?.slack_mapped ? '<span class="muted small">slack</span>' : ""}</li>`;
      });
      people.innerHTML = rows.join("") || `<li class="muted">—</li>`;
      people.querySelectorAll("[data-node]").forEach((b) => {
        b.addEventListener("click", () => {
          const id = b.getAttribute("data-node");
          const n = graphState.nodes.find((x) => x.id === id);
          if (n) {
            graphState.selected = id;
            renderGraphDetail(n);
            drawGraph();
          }
        });
      });
    }
  }
  const edgesEl = $("graph-edges");
  if (edgesEl) {
    const sample = graphState.edges.slice(-25).reverse();
    edgesEl.innerHTML = sample.length
      ? sample
          .map(
            (e) =>
              `<li>${esc(e.type)} <span class="muted">${prettyRef(e.from)} → ${prettyRef(e.to)}</span></li>`
          )
          .join("")
      : `<li class="muted">No edges yet</li>`;
  }
  const counts = $("graph-type-counts");
  if (counts) {
    const by = data.by_type || {};
    if (isSimpleMode()) {
      const friendly = {
        Commit: "updates",
        Person: "people",
        Repo: "projects",
        PullRequest: "reviews",
        Intent: "focuses",
        Issue: "issues",
      };
      counts.className = "meta-row";
      counts.innerHTML = Object.entries(by)
        .map(([k, v]) => `<span class="pill mid">${v} ${esc(friendly[k] || k)}</span>`)
        .join(" ") || `<span class="muted">Nothing on the map yet.</span>`;
    } else {
      counts.className = "box small";
      counts.textContent = JSON.stringify(
        {
          nodes: by,
          edges: data.edge_by_type || {},
        },
        null,
        2
      );
    }
  }
  if (graphState.selected) {
    const n = graphState.nodes.find((x) => x.id === graphState.selected);
    if (n) renderGraphDetail(n);
  }
}

function shortId(id) {
  if (!id) return "";
  if (id.length <= 28) return id;
  return id.slice(0, 12) + "…" + id.slice(-8);
}

function renderGraphDetail(n) {
  const el = $("graph-detail");
  if (!el || !n) return;
  const linked = graphState.edges.filter((e) => e.from === n.id || e.to === n.id);
  const neighbors = linked.map((e) => (e.from === n.id ? e.to : e.from));
  const uniq = [...new Set(neighbors)];
  const intent = n.meta?.intent_type || "";
  const byId = new Map(graphState.nodes.map((x) => [x.id, x]));
  const title = n.label && !isOpaqueToken(n.label) ? n.label : displayLabel(n.meta || n, n.type);
  el.innerHTML = `
    <div class="meta-row">
      <span class="graph-node-chip">${esc(n.type)}</span>
      ${intent ? `<span class="graph-node-chip">${esc(intent)}</span>` : ""}
      ${n.meta?.from_team_map ? `<span class="graph-node-chip">team map</span>` : ""}
      ${idEye(n.id)}
    </div>
    <p style="margin:0.6rem 0 0.2rem;font-weight:600;">${esc(title)}</p>
    ${n.meta?.resource_id && isOpaqueToken(n.meta.resource_id) ? `<p class="muted small">${prettyRef(n.meta.resource_id)}</p>` : n.meta?.resource_id && !isOpaqueToken(n.meta.resource_id) ? `<p class="muted small">${esc(n.meta.resource_id)}</p>` : ""}
    <p class="muted small" style="margin-top:0.75rem;">${linked.length} edge(s) · ${uniq.length} neighbor(s)</p>
    <ul class="item-list">
      ${linked
        .slice(0, 12)
        .map((e) => {
          const other = e.from === n.id ? e.to : e.from;
          const dir = e.from === n.id ? "→" : "←";
          const on = byId.get(other);
          const onLab = on
            ? displayLabel(on.meta || on, on.type)
            : idKindLabel(other);
          return `<li>${esc(e.type)} ${dir} ${esc(onLab)} ${idEye(other)}</li>`;
        })
        .join("") || "<li class='muted'>No edges</li>"}
    </ul>
  `;
}

function stepForce() {
  graphState.frame = (graphState.frame || 0) + 1;
  const nodes = graphState.nodes.filter((n) => nodeVisible(n));
  if (nodes.length === 0) return;
  const byId = new Map(graphState.nodes.map((n) => [n.id, n]));
  const edges = graphState.edges.filter((e) => {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    return a && b && nodeVisible(a) && nodeVisible(b);
  });

  // Soft anchors (hierarchical story stays readable)
  for (const n of nodes) {
    if (n.fx != null || !n.pinSoft) continue;
    const k = n.pinSoft.k || 0.03;
    n.vx += (n.pinSoft.x - n.x) * k;
    n.vy += (n.pinSoft.y - n.y) * k;
  }

  // Repulsion — skip commit↔commit pairs far apart (perf + less explosion)
  const nCount = nodes.length;
  const repulse = nCount > 40 ? 420 : 700;
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i];
      const b = nodes[j];
      let dx = b.x - a.x;
      let dy = b.y - a.y;
      let dist2 = dx * dx + dy * dy || 0.01;
      if (a.type === "Commit" && b.type === "Commit" && dist2 > 120 * 120) continue;
      if (dist2 > 380 * 380) continue;
      const dist = Math.sqrt(dist2);
      const minD = a.r + b.r + (a.type === "Commit" || b.type === "Commit" ? 10 : 36);
      let force = repulse / dist2;
      if (dist < minD) force += (minD - dist) * 0.22;
      if (a.type === "Person" && b.type === "Person") force *= 0.4;
      if (a.type === "Commit" && b.type === "Commit") force *= 0.35;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      a.vx -= fx;
      a.vy -= fy;
      b.vx += fx;
      b.vy += fy;
    }
  }

  // Springs
  for (const e of edges) {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    if (!a || !b) continue;
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    const dist = Math.sqrt(dx * dx + dy * dy) || 0.01;
    let ideal = 110;
    let k = 0.04;
    if (e.type === "CLAIMS" || e.type === "ABOUT") {
      ideal = 70;
      k = 0.06;
    }
    if (e.type === "BLOCKS" || e.type === "BLOCKED_BY") {
      ideal = 95;
      k = 0.055;
    }
    if (e.type === "AUTHORED") {
      ideal = a.type === "Commit" || b.type === "Commit" ? 55 : 90;
      k = 0.05;
    }
    if (e.type === "PUSHED_TO") {
      ideal = 130;
      k = 0.025;
    }
    if (e.type === "BELONGS_TO" || e.type === "CHECKED") {
      ideal = 100;
      k = 0.03;
    }
    // Don't let dozens of PUSHED_TO stretch the whole map
    if (e.type === "PUSHED_TO" && (a.type === "Commit" || b.type === "Commit")) continue;
    const f = (dist - ideal) * k;
    const fx = (dx / dist) * f;
    const fy = (dy / dist) * f;
    a.vx += fx;
    a.vy += fy;
    b.vx -= fx;
    b.vy -= fy;
  }

  // Mild center gravity
  for (const n of nodes) {
    n.vx += -n.x * 0.0012;
    n.vy += -n.y * 0.0012;
  }

  // Integrate with velocity cap
  const damp = graphState.frame > 90 ? 0.78 : 0.84;
  for (const n of graphState.nodes) {
    if (!nodeVisible(n)) continue;
    if (n.fx != null) {
      n.x = n.fx;
      n.y = n.fy;
      n.vx = 0;
      n.vy = 0;
      continue;
    }
    n.vx *= damp;
    n.vy *= damp;
    const sp = Math.hypot(n.vx, n.vy);
    if (sp > 8) {
      n.vx = (n.vx / sp) * 8;
      n.vy = (n.vy / sp) * 8;
    }
    n.x += n.vx;
    n.y += n.vy;
  }
}

function fitGraph(opts = {}) {
  const hubsOnly = opts.hubsOnly !== false;
  const minScale = opts.minScale ?? 0.65;
  const maxScale = opts.maxScale ?? 1.35;
  let nodes = graphState.nodes.filter((n) => nodeVisible(n));
  if (hubsOnly) {
    const hubs = nodes.filter((n) => GRAPH_HUB_TYPES.has(n.type));
    if (hubs.length >= 2) nodes = hubs;
    else if (hubs.length === 1 && nodes.length > 8) {
      // one hub + a few nearby commits
      nodes = hubs.concat(nodes.filter((n) => n.type === "Commit").slice(0, 6));
    }
  }
  const canvas = $("graph-canvas");
  if (!canvas || !nodes.length) {
    graphState.pan = { x: canvas ? canvas.width / 2 : 0, y: canvas ? canvas.height / 2 : 0 };
    graphState.scale = 1;
    return;
  }
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const n of nodes) {
    minX = Math.min(minX, n.x - n.r - 24);
    minY = Math.min(minY, n.y - n.r - 28);
    maxX = Math.max(maxX, n.x + n.r + 24);
    maxY = Math.max(maxY, n.y + n.r + 36);
  }
  const w = Math.max(120, maxX - minX);
  const h = Math.max(120, maxY - minY);
  const pad = 72;
  const sx = (canvas.width - pad * 2) / w;
  const sy = (canvas.height - pad * 2) / h;
  // Never aggressively zoom out below minScale
  graphState.scale = Math.min(maxScale, Math.max(minScale, Math.min(sx, sy)));
  const cx = (minX + maxX) / 2;
  const cy = (minY + maxY) / 2;
  graphState.pan.x = canvas.width / 2 - cx * graphState.scale;
  graphState.pan.y = canvas.height / 2 - cy * graphState.scale;
}

function drawGraph() {
  const canvas = $("graph-canvas");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const dpr = window.devicePixelRatio || 1;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, canvas.width, canvas.height);

  ctx.save();
  ctx.translate(graphState.pan.x, graphState.pan.y);
  ctx.scale(graphState.scale, graphState.scale);

  const byId = new Map(graphState.nodes.map((n) => [n.id, n]));
  const selected = graphState.selected;
  const visibleNodes = graphState.nodes.filter((n) => nodeVisible(n));
  const visibleEdges = graphState.edges.filter((e) => {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    return a && b && nodeVisible(a) && nodeVisible(b);
  });

  // Draw non-selected edges first (dim), then story edges, then selected neighborhood
  const edgePriority = (e) => {
    if (e.type === "BLOCKS" || e.type === "BLOCKED_BY") return 3;
    if (e.type === "CLAIMS" || e.type === "ABOUT") return 2;
    if (e.type === "AUTHORED" || e.type === "ASSIGNED_TO") return 1;
    return 0;
  };
  const sorted = [...visibleEdges].sort((a, b) => edgePriority(a) - edgePriority(b));

  for (const e of sorted) {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    if (!a || !b) continue;
    const inSel = selected && (a.id === selected || b.id === selected);
    // Skip most PUSHED_TO clutter unless selected
    if (e.type === "PUSHED_TO" && !inSel && visibleEdges.length > 25) continue;

    const isBlock = /block/i.test(e.type);
    const isClaim = e.type === "CLAIMS" || e.type === "ABOUT";
    ctx.beginPath();
    // slight curve for readability
    const mx = (a.x + b.x) / 2 + (a.y - b.y) * 0.08;
    const my = (a.y + b.y) / 2 + (b.x - a.x) * 0.08;
    ctx.moveTo(a.x, a.y);
    ctx.quadraticCurveTo(mx, my, b.x, b.y);
    let alpha = inSel ? 1 : selected ? 0.18 : 0.55;
    if (isBlock) alpha = Math.max(alpha, 0.9);
    if (isClaim) alpha = Math.max(alpha, 0.75);
    ctx.strokeStyle = isBlock
      ? `rgba(181,106,106,${alpha})`
      : isClaim
        ? `rgba(59,111,120,${alpha})`
        : `rgba(168,166,160,${alpha})`;
    ctx.lineWidth = (isBlock ? 2.2 : inSel ? 1.6 : 1.1) / graphState.scale;
    if (isClaim) ctx.setLineDash([5 / graphState.scale, 4 / graphState.scale]);
    else if (isBlock) ctx.setLineDash([3 / graphState.scale, 3 / graphState.scale]);
    else ctx.setLineDash([]);
    ctx.stroke();
    ctx.setLineDash([]);

    const showLabel =
      isBlock ||
      isClaim ||
      inSel ||
      (visibleEdges.length < 30 && edgePriority(e) >= 1);
    if (showLabel) {
      ctx.font = `${10 / graphState.scale}px ui-sans-serif, system-ui, sans-serif`;
      ctx.fillStyle = inSel || isBlock ? "#6b6b6b" : "#a8a6a0";
      ctx.textAlign = "center";
      ctx.fillText(e.type, mx, my - 4 / graphState.scale);
    }
  }

  // Nodes: commits first (under), hubs on top
  const paintOrder = [...visibleNodes].sort((a, b) => {
    const rank = (t) =>
      t === "Commit" ? 0 : t === "Repo" ? 1 : t === "Intent" ? 3 : t === "Person" ? 4 : 2;
    return rank(a.type) - rank(b.type);
  });

  for (const n of paintOrder) {
    const isSel = n.id === selected;
    const dim = selected && !isSel;
    ctx.globalAlpha = dim ? 0.35 : 1;
    drawNodeShape(ctx, n, isSel);
    const showLabel =
      GRAPH_HUB_TYPES.has(n.type) ||
      isSel ||
      graphState.scale >= 1.05 ||
      visibleNodes.filter((x) => x.type === "Commit").length <= 10;
    if (showLabel) {
      ctx.font = `${(n.type === "Person" ? 12 : 11) / graphState.scale}px ui-sans-serif, system-ui, sans-serif`;
      ctx.fillStyle = "#1a1a1a";
      ctx.textAlign = "center";
      const maxLen = n.type === "Commit" ? 28 : n.type === "Person" ? 20 : 26;
      ctx.fillText(truncateLabel(n.label, maxLen), n.x, n.y + n.r + 14 / graphState.scale);
      if (n.type === "Intent" && n.meta?.intent_type) {
        ctx.fillStyle = "#8a8a8a";
        ctx.font = `${9 / graphState.scale}px Inter, ui-sans-serif, system-ui, sans-serif`;
        ctx.fillText(plainIntentType(n.meta.intent_type), n.x, n.y + n.r + 24 / graphState.scale);
      }
    }
    ctx.globalAlpha = 1;
  }

  ctx.restore();

  // HUD: visible vs total
  const total = graphState.nodes.length;
  const vis = visibleNodes.length;
  if (total > vis) {
    ctx.fillStyle = "#737373";
    ctx.font = `${11 * dpr}px ui-sans-serif, system-ui, sans-serif`;
    ctx.textAlign = "left";
    ctx.fillText(
      `Showing ${vis}/${total} nodes (recent commits / filters)`,
      12 * dpr,
      20 * dpr
    );
  }

  if (!graphState.nodes.length) {
    const raw = graphState.raw || {};
    const v2Up = raw.v2_up !== false && raw.error !== "v2_unreachable";
    ctx.fillStyle = "#737373";
    ctx.font = `${14 * dpr}px ui-sans-serif, system-ui, sans-serif`;
    ctx.textAlign = "center";
    const msg = !v2Up
      ? "The map service is recovering — this will refill on its own"
      : "The map is still filling from recent work";
    ctx.fillText(msg, canvas.width / 2, canvas.height / 2);
  }
}

function drawNodeShape(ctx, n, selected) {
  const r = n.r;
  ctx.save();
  ctx.translate(n.x, n.y);
  if (selected) {
    ctx.beginPath();
    ctx.arc(0, 0, r + 5, 0, Math.PI * 2);
    ctx.strokeStyle = "#3b6f78";
    ctx.lineWidth = 2 / graphState.scale;
    ctx.stroke();
  }
  ctx.lineWidth = 1.4 / graphState.scale;
  ctx.strokeStyle = "#1c1b17";
  ctx.fillStyle = "#ffffff";

  if (n.type === "Person") {
    ctx.beginPath();
    ctx.arc(0, 0, r, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(0, -r * 0.25, r * 0.28, 0, Math.PI * 2);
    ctx.fillStyle = "#1c1b17";
    ctx.fill();
    ctx.beginPath();
    ctx.arc(0, r * 0.55, r * 0.55, Math.PI, 0);
    ctx.fill();
  } else if (n.type === "PullRequest") {
    ctx.fillStyle = "#1c1b17";
    ctx.fillRect(-r * 0.7, -r * 0.7, r * 1.4, r * 1.4);
    ctx.strokeRect(-r * 0.7, -r * 0.7, r * 1.4, r * 1.4);
  } else if (n.type === "Issue" || n.type === "Ticket") {
    ctx.beginPath();
    ctx.moveTo(0, -r);
    ctx.lineTo(r, 0);
    ctx.lineTo(0, r);
    ctx.lineTo(-r, 0);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
  } else if (n.type === "StatusDigest") {
    // Diamond for digest decision (local coords after translate)
    const d = n.meta?.decision || n.decision || n.intent_type || "";
    ctx.beginPath();
    ctx.moveTo(0, -r);
    ctx.lineTo(r, 0);
    ctx.lineTo(0, r);
    ctx.lineTo(-r, 0);
    ctx.closePath();
    ctx.fillStyle =
      d === "approved" ? "#eef3ef" : d === "dont_send" ? "#f6eeee" : "#f3f3f1";
    ctx.fill();
    ctx.strokeStyle = d === "approved" ? "#5f8466" : d === "dont_send" ? "#b56a6a" : "#8a8a8a";
    ctx.lineWidth = selected ? 2.5 : 1.5;
    ctx.stroke();
  } else if (n.type === "Intent") {
    ctx.setLineDash([3 / graphState.scale, 2 / graphState.scale]);
    ctx.strokeRect(-r * 0.75, -r * 0.75, r * 1.5, r * 1.5);
    ctx.setLineDash([]);
    ctx.fillStyle = "#fafafa";
    ctx.fillRect(-r * 0.75, -r * 0.75, r * 1.5, r * 1.5);
    ctx.strokeRect(-r * 0.75, -r * 0.75, r * 1.5, r * 1.5);
  } else if (n.type === "Repo") {
    ctx.beginPath();
    ctx.arc(0, 0, r, 0, Math.PI * 2);
    ctx.fillStyle = "#e8e8e6";
    ctx.fill();
    ctx.stroke();
  } else {
    ctx.beginPath();
    ctx.arc(0, 0, r * 0.8, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
  }
  ctx.restore();
}

function truncateLabel(s, n) {
  const t = String(s || "");
  return t.length > n ? t.slice(0, n - 1) + "…" : t;
}

$("btn-refresh").addEventListener("click", async () => {
  await refreshHealth();
  await refreshOnboarding();
  await loadLatest();
  await refreshPulse();
  await refreshMetrics();
  await refreshReadiness();
  if (!$("view-cockpit")?.classList.contains("hidden")) {
    await refreshCockpit();
  }
  if (!$("view-graph")?.classList.contains("hidden")) {
    await refreshGraph(false);
  }
  if (!$("view-insights")?.classList.contains("hidden")) {
    await refreshDevInsights();
  }
});
$("btn-sim").addEventListener("click", simulate);
$("btn-publish").addEventListener("click", () => act("publish"));
$("btn-veto").addEventListener("click", () => act("veto"));
$("btn-silence").addEventListener("click", () => act("silence"));
$("btn-edit").addEventListener("click", () => act("edit"));
if ($("btn-slack-oauth")) {
  $("btn-slack-oauth").addEventListener("click", () => startOAuth("slack"));
}
if ($("btn-teams-connect")) {
  $("btn-teams-connect").addEventListener("click", () => startOAuth("teams"));
}
if ($("btn-gh-app")) {
  $("btn-gh-app").addEventListener("click", () => startOAuth("github"));
}
if ($("ck-tomorrow-save")) {
  $("ck-tomorrow-save").addEventListener("click", () => saveTomorrowFocus(false));
}
if ($("ck-tomorrow-clear")) {
  $("ck-tomorrow-clear").addEventListener("click", () => saveTomorrowFocus(true));
}
if ($("btn-roles-save")) {
  $("btn-roles-save").addEventListener("click", () => saveRoles());
}
if ($("btn-roles-reload")) {
  $("btn-roles-reload").addEventListener("click", () => reloadRoles());
}
async function refreshEvents() {
  const tenant = syncTenantFields(activeTenant());
  const el = $("events-log");
  const meta = $("events-meta");
  try {
    const [e, obs] = await Promise.all([
      jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/events?limit=80`),
      jfetch("/v3/observe/status").catch(() => ({})),
    ]);
    if (meta) {
      const gm = obs.graph_mirror ? " · graph_mirror=true" : "";
      meta.textContent = `count=${e.count} · external_db=${e.external_db} · env_set=${obs.env_url_set}${gm} · ${e.note || ""}`;
    }
    if (el) el.textContent = JSON.stringify(e.events || [], null, 2);
  } catch (err) {
    if (meta) meta.textContent = "events: " + (err.message || err);
  }
}
async function syncTwinToDb() {
  const tenant = syncTenantFields(activeTenant());
  const meta = $("events-meta");
  try {
    if (meta) meta.textContent = "Syncing Docker twin state → Neon…";
    const body = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/sync_to_db`,
      { method: "POST", body: "{}" }
    );
    if (meta) {
      meta.textContent = `synced twins=${body.twins} maps=${body.slack_maps} drafts=${body.drafts} kv=${body.tenant_kv} @ ${body.synced_at}`;
    }
    alert(
      "Re-synced to Neon (upsert, no wipe):\n" +
        JSON.stringify(body, null, 2) +
        "\n\nNew product data dual-writes continuously — you usually do not need this button."
    );
    await refreshEvents();
  } catch (err) {
    alert("Sync failed: " + (err.message || err));
    if (meta) meta.textContent = "sync failed: " + (err.message || err);
  }
}
async function syncGraphToDb() {
  const tenant = syncTenantFields(activeTenant());
  const meta = $("events-meta");
  try {
    if (meta) meta.textContent = "Exporting V2 graph → Neon…";
    const body = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/sync_graph_to_db`,
      { method: "POST", body: "{}" }
    );
    if (meta) {
      meta.textContent = `graph export nodes=${body.nodes} edges=${body.edges} @ ${body.synced_at || ""}`;
    }
    alert(
      "Graph exported to Neon (upsert + orphan delete):\n" +
        JSON.stringify(body, null, 2) +
        "\n\nAlso runs periodically when OBSERVE_DATABASE_URL is set. Graph UI remains primary."
    );
    await refreshEvents();
  } catch (err) {
    alert("Graph export failed: " + (err.message || err));
    if (meta) meta.textContent = "graph export failed: " + (err.message || err);
  }
}
if ($("btn-events-refresh")) {
  $("btn-events-refresh").addEventListener("click", () => refreshEvents());
}
if ($("btn-sync-db")) {
  $("btn-sync-db").addEventListener("click", () => syncTwinToDb());
}
if ($("btn-sync-graph-db")) {
  $("btn-sync-graph-db").addEventListener("click", () => syncGraphToDb());
}
async function refreshStackHealth() {
  const pills = $("stack-health-pills");
  const raw = $("stack-health-raw");
  try {
    const d = await jfetch("/v3/demo/status");
    const row = (name, up) =>
      `<span class="pill ${up ? "up" : "down"}">${name}: ${up ? "up" : "down"}</span>`;
    if (pills) {
      pills.innerHTML = [
        row("v1", d.v1 === true),
        row("v2", d.v2 === true),
        row("v3/twin", d.v3 === true),
        row("egress", d.egress === true),
        `<span class="pill mid">graph: ${esc(d.graph_status || "?")} (${d.graph_nodes || 0}n/${d.graph_edges || 0}e)</span>`,
        `<span class="pill mid">mode: ${esc(d.mode || "?")}</span>`,
        `<span class="pill mid">slack: ${esc(d.slack_mode || "?")}</span>`,
      ].join(" ");
    }
    if (raw) {
      raw.textContent = JSON.stringify(
        {
          v1: d.v1,
          v2: d.v2,
          v3: d.v3,
          egress: d.egress,
          graph_status: d.graph_status,
          graph_nodes: d.graph_nodes,
          graph_edges: d.graph_edges,
          durability: d.durability,
          slack_mode: d.slack_mode,
          delivery_adapter: d.delivery_adapter,
        },
        null,
        2
      );
    }
  } catch (e) {
    if (pills) pills.innerHTML = `<span class="pill down">probe failed: ${esc(e.message || e)}</span>`;
  }
}
if ($("btn-stack-health")) {
  $("btn-stack-health").addEventListener("click", () => refreshStackHealth());
}
if ($("btn-team-refresh")) {
  $("btn-team-refresh").addEventListener("click", refreshTeam);
}
if ($("btn-insights-refresh")) {
  $("btn-insights-refresh").addEventListener("click", () => refreshDevInsights());
}
if ($("btn-team-compile")) {
  $("btn-team-compile").addEventListener("click", compileTeamDigests);
}
async function pruneTeamDuplicates() {
  const tenant = $("team-tenant")?.value?.trim() || "ten_github";
  const msg = $("team-compile-msg");
  try {
    const out = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team/prune`, {
      method: "POST",
      body: "{}",
    });
    if (msg) {
      msg.textContent = `Pruned ${out.pruned ?? 0} duplicate twin(s). Enabled people: ${out.enabled_person_twins ?? "—"}.`;
    }
    await refreshTeam();
    await refreshGraph(true).catch(() => {});
  } catch (e) {
    if (msg) msg.textContent = "Prune failed (deploy latest if 404): " + (e.message || e);
  }
}
if ($("btn-team-prune")) {
  $("btn-team-prune").addEventListener("click", pruneTeamDuplicates);
}
async function seedIntentDemo() {
  const tenant =
    $("team-tenant")?.value?.trim() ||
    $("tenant")?.value?.trim() ||
    "ten_github";
  const msg = $("seed-intent-msg");
  const btn = $("btn-seed-intent");
  if (btn) {
    btn.disabled = true;
    btn.textContent = "Seeding…";
  }
  try {
    const out = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/seed/intent_demo`,
      { method: "POST", body: "{}" }
    );
    if (msg) {
      msg.textContent = `Seeded: ${out.conflict_count ?? "?"} conflict(s), ${out.intent_count ?? "?"} intent(s). Refreshing…`;
    }
    await refreshPulse();
    await refreshGraph(true).catch(() => {});
  } catch (e) {
    if (msg) msg.textContent = "Seed failed: " + (e.message || e);
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "Load intent demo (SHIP vs FREEZE)";
    }
  }
}
if ($("btn-seed-intent")) {
  $("btn-seed-intent").addEventListener("click", seedIntentDemo);
}
if ($("btn-seed-story")) {
  $("btn-seed-story").addEventListener("click", async () => {
    const msg = $("seed-intent-msg");
    const btn = $("btn-seed-story");
    if (btn) {
      btn.disabled = true;
      btn.textContent = "Enriching…";
    }
    try {
      await enrichGraphStory();
      if (msg) msg.textContent = "Real-team story seeded (PR + SHIP/FREEZE). Open Graph.";
      await refreshPulse();
      await refreshReadiness();
    } catch (e) {
      if (msg) msg.textContent = "Story seed failed: " + (e.message || e);
    } finally {
      if (btn) {
        btn.disabled = false;
        btn.textContent = "Enrich real-team story";
      }
    }
  });
}
// ─── Person profile (Cockpit + Lab) ─────────────────────────────────────────

function profileSubject() {
  return (
    $("ck-profile-subject")?.value?.trim() ||
    $("lab-profile-subject")?.value?.trim() ||
    "neeljoshi18"
  );
}

function renderPersonProfile(p, hostEl) {
  if (!hostEl || !p) return;
  const sub = p.subject || {};
  const ws = p.work_surface || {};
  const cadence = p.cadence || {};
  const digests = p.digests?.latest || [];
  const intents = p.intents || [];
  const conflicts = p.conflicts_touching || [];
  const ftItems = p.follow_through?.items || [];
  const claims = p.slack_intent_claims || [];
  const unknown = p.what_we_cannot_know || [];

  const repos = (ws.repos || [])
    .slice(0, 12)
    .map((r) => `<li><strong>${esc(r.repo || r)}</strong> <span class="muted small">${esc(String(r.commit_touches ?? ""))}</span></li>`)
    .join("") || `<li class="muted">No repos linked yet</li>`;
  const commits = (ws.commit_sample || [])
    .slice(0, 8)
    .map(
      (c) =>
        `<li>${prettyRef(c.sha7 || c.id || "")} ${scrubTextHtml((c.message || "").slice(0, 90))}</li>`
    )
    .join("") || `<li class="muted">No commit sample</li>`;
  const digHtml = digests.length
    ? digests
        .slice(0, 3)
        .map(
          (d) =>
            `<li><span class="pill mid">${esc(d.status || "")}</span> <pre class="box small" style="white-space:pre-wrap;max-height:120px;overflow:auto;margin:0.25rem 0 0;">${scrubTextHtml((d.preview || d.draft_text || "").slice(0, 500))}</pre></li>`
        )
        .join("")
    : `<li class="muted">No digests for this twin yet — compile digests first</li>`;
  const intentHtml = intents.length
    ? intents
        .slice(0, 12)
        .map((i) => {
          const ty = i.intent_type || i.properties?.intent_type || "Intent";
          const lab = i.display_name || i.label || i.title || i.id || "";
          const demo = i.is_demo ? ` <span class="pill mid">demo</span>` : "";
          return `<li><strong>${esc(ty)}</strong> ${prettyMaybe(lab) || prettyRef(i.id)}${demo}</li>`;
        })
        .join("")
    : `<li class="muted">No person-owned intents</li>`;
  const confHtml = conflicts.length
    ? conflicts
        .slice(0, 10)
        .map(
          (c) =>
            `<li><strong>[${esc(c.severity || c.kind || "?")}]</strong> ${scrubTextHtml(c.summary || c.kind || "")}${c.is_demo ? ' <span class="pill mid">demo</span>' : ""}</li>`
        )
        .join("")
    : `<li class="muted">No conflicts touching this person</li>`;
  const ftHtml = ftItems.length
    ? ftItems
        .slice(0, 12)
        .map((it) => {
          const st = it.status || "unknown";
          const pillCls =
            st === "supported" ? "up" : st === "contradicted" || st === "abandoned" ? "down" : "mid";
          return `<li><span class="pill ${pillCls}">${esc(st)}</span> <strong>${esc(it.intent_type || "")}</strong> ${scrubTextHtml(it.said_or_implied || "")}<div class="muted small">${scrubTextHtml(it.gap || "")}</div></li>`;
        })
        .join("")
    : `<li class="muted">No aged non-demo intents to score yet</li>`;
  const claimsHtml = claims.length
    ? claims
        .slice(0, 12)
        .map(
          (c) =>
            `<li><span class="pill mid">${esc(c.intent_type || "?")}</span> <span class="muted small">${prettyMaybe(c.channel || "")} · conf ${esc(String(c.confidence ?? ""))}</span><div>${scrubTextHtml(c.text_preview || "")}</div></li>`
        )
        .join("")
    : `<li class="muted">No channel/DM intent claims yet — invite bot to a team channel; not a private wiretap</li>`;
  const unkHtml = unknown
    .map((u) => `<li class="muted">${esc(u)}</li>`)
    .join("");

  hostEl.innerHTML = `
    <div class="meta-row" style="margin-bottom:0.5rem;">
      <span class="pill up">${displayNameOrEye(sub.display_name, sub.subject_id)}</span>
      ${sub.subject_id ? prettyRef(sub.subject_id) : ""}
      <span class="pill mid">confidence ${esc(String(Math.round((p.confidence_overall || 0) * 100)))}%</span>
      <span class="pill mid">${esc(fmtIst(p.as_of))}</span>
    </div>
    <p class="muted small">${esc(p.doctrine || "")}</p>
    <div class="cockpit-grid" style="margin-top:0.5rem;">
      <div>
        <h3 class="graph-side-h">Work surface</h3>
        <ul class="item-list">${repos}</ul>
        <h3 class="graph-side-h">Commit sample</h3>
        <ul class="item-list">${commits}</ul>
      </div>
      <div>
        <h3 class="graph-side-h">Cadence</h3>
        <p class="muted small">${esc(cadence.notes || "")}</p>
        <p class="muted small">Peak hour IST: <strong>${esc(String(cadence.peak_hour_ist ?? cadence.peak_hour_utc ?? "—"))}</strong> (${esc(String(cadence.peak_count ?? 0))})</p>
        <h3 class="graph-side-h">Digests</h3>
        <ul class="item-list">${digHtml}</ul>
      </div>
    </div>
    <div class="cockpit-grid" style="margin-top:0.5rem;">
      <div>
        <h3 class="graph-side-h">Intents</h3>
        <ul class="item-list">${intentHtml}</ul>
        <h3 class="graph-side-h">Conflicts touching</h3>
        <ul class="item-list">${confHtml}</ul>
      </div>
      <div>
        <h3 class="graph-side-h">Follow-through</h3>
        <p class="muted small">${esc(p.follow_through?.note || "")}</p>
        <ul class="item-list">${ftHtml}</ul>
        <h3 class="graph-side-h">Slack intent claims</h3>
        <p class="muted small">Channel claims only when bot is invited · bot DMs for free-text · never private 1:1 wiretap</p>
        <ul class="item-list">${claimsHtml}</ul>
      </div>
    </div>
    <h3 class="graph-side-h">What we cannot know</h3>
    <ul class="item-list">${unkHtml}</ul>
    <p class="muted small" style="margin-top:0.5rem;">${esc(p.note || "")}</p>
  `;
}

async function loadPersonProfile(opts) {
  const intoLab = opts?.intoLab === true;
  const tenant = syncTenantFields(activeTenant());
  const subject = intoLab
    ? $("lab-profile-subject")?.value?.trim() || profileSubject()
    : profileSubject();
  const msg = intoLab ? $("lab-profile-msg") : $("ck-profile-msg");
  const host = intoLab ? null : $("ck-profile-body");
  if (msg) msg.textContent = `Loading profile for ${subject}…`;
  // keep inputs in sync
  if ($("ck-profile-subject") && subject) $("ck-profile-subject").value = subject;
  if ($("lab-profile-subject") && subject) $("lab-profile-subject").value = subject;
  try {
    const p = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/people/${encodeURIComponent(subject)}/profile`
    );
    if (host) renderPersonProfile(p, host);
    if ($("lab-profile-raw")) {
      $("lab-profile-raw").textContent = JSON.stringify(p, null, 2);
    }
    if ($("lab-raw") && intoLab) {
      $("lab-raw").textContent = JSON.stringify(
        { profile_subject: subject, as_of: p.as_of, confidence: p.confidence_overall },
        null,
        2
      );
    }
    if (msg) {
      msg.textContent = `Profile · ${p.subject?.display_name || subject} · conf ${Math.round((p.confidence_overall || 0) * 100)}% · channel intents only when bot invited (not private wiretap)`;
    }
    return p;
  } catch (e) {
    if (msg) msg.textContent = "Profile failed: " + (e.message || e);
    if (host) host.innerHTML = `<p class="muted">Failed to load profile.</p>`;
    throw e;
  }
}

async function loadFollowThroughOnly() {
  const tenant = syncTenantFields(activeTenant());
  const subject = profileSubject();
  const msg = $("ck-profile-msg");
  const host = $("ck-profile-body");
  if (msg) msg.textContent = `Loading follow-through for ${subject}…`;
  try {
    const ft = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/people/${encodeURIComponent(subject)}/follow_through`
    );
    const items = ft.items || [];
    if (host) {
      host.innerHTML = `
        <div class="meta-row"><span class="pill mid">${prettyMaybe(ft.subject_id || subject)}</span>
        <span class="pill mid">${items.length} item(s)</span></div>
        <p class="muted small">${esc(ft.note || "")}</p>
        <ul class="item-list">${
          items.length
            ? items
                .map((it) => {
                  const st = it.status || "unknown";
                  const pillCls =
                    st === "supported"
                      ? "up"
                      : st === "contradicted" || st === "abandoned"
                        ? "down"
                        : "mid";
                  return `<li><span class="pill ${pillCls}">${esc(st)}</span> <strong>${esc(it.intent_type || "")}</strong> ${scrubTextHtml(it.said_or_implied || "")}<div class="muted small">${scrubTextHtml(it.gap || "")}</div></li>`;
                })
                .join("")
            : `<li class="muted">No follow-through items</li>`
        }</ul>`;
    }
    if (msg) msg.textContent = `Follow-through · ${items.length} item(s)`;
  } catch (e) {
    if (msg) msg.textContent = "Follow-through failed: " + (e.message || e);
  }
}

async function loadIntentLedger() {
  const tenant = syncTenantFields(activeTenant());
  const demo = $("ck-intent-demo")?.checked === true;
  const list = $("ck-intent-ledger");
  const stats = $("ck-intent-stats");
  const note = $("ck-intent-note");
  const pill = $("ck-intent-pill");
  try {
    const [eng, led] = await Promise.all([
      jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/intent/engine`).catch(() => null),
      jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/intent/ledger?include_demo=${demo}&open_only=true&limit=40`
      ),
    ]);
    const s = led.stats || {};
    const live = s.live ?? "—";
    const dem = s.demo ?? "—";
    const total = s.total ?? led.count ?? 0;
    const simple = isSimpleMode();
    if (stats) {
      stats.innerHTML = simple
        ? [
            `<span class="pill up">${esc(String(live))} live focus item${live === 1 ? "" : "s"}</span>`,
            dem > 0
              ? `<span class="pill mid">${esc(String(dem))} demo (ignore for real decisions)</span>`
              : "",
            `<span class="pill mid">showing ${esc(String(total))}</span>`,
          ]
            .filter(Boolean)
            .join(" ")
        : [
            `<span class="pill up">live claims: ${esc(String(live))}</span>`,
            `<span class="pill mid">demo: ${esc(String(dem))}</span>`,
            `<span class="pill mid">shown: ${esc(String(total))}</span>`,
            eng?.conflicts_cached
              ? `<span class="pill mid">conflicts: ${esc(String(eng.conflicts_cached.count ?? 0))}</span>`
              : "",
          ]
            .filter(Boolean)
            .join(" ");
    }
    if (pill) {
      pill.textContent = simple ? "focus list" : eng?.in_house === false ? "external?" : "in-house";
      pill.className = "pill up";
    }
    const claims = led.claims || [];
    if (list) {
      if (!claims.length) {
        list.innerHTML = simple
          ? `<li class="muted">No open focus items yet. Capture a claim, or wait for Slack/GitHub signals.</li>`
          : `<li class="muted">No open live claims yet — capture via bot/channel keywords or Capture claim. Demo seeds hidden unless checked.</li>`;
      } else if (simple) {
        list.innerHTML = claims
          .map((c) => {
            const ty = c.intent_type || "OTHER";
            const sum = (c.summary || c.text_preview || "")
              .replace(/^(SHIP|BLOCKED|FREEZE|FIX|EXPLORE|REVIEW):\s*/i, "")
              .slice(0, 140);
            const own = c.owner_subject ? prettyMaybe(c.owner_subject) : "Someone";
            const demoTag = c.is_demo ? ` <span class="pill mid">demo</span>` : "";
            return `<li class="ux-card"><span class="pill mid">${esc(plainIntentType(ty))}</span>${demoTag}
              <strong>${own}</strong> — ${scrubTextHtml(sum || plainIntentType(ty))}</li>`;
          })
          .join("");
      } else {
        list.innerHTML = claims
          .map((c) => {
            const ty = c.intent_type || "OTHER";
            const src = c.source || "?";
            const demoTag = c.is_demo ? ` <span class="pill mid">demo</span>` : "";
            const conf = typeof c.confidence === "number" ? Math.round(c.confidence * 100) + "%" : "";
            const sum = c.summary || c.text_preview || "";
            const own = c.owner_subject ? prettyMaybe(c.owner_subject) : "—";
            return `<li><span class="pill mid">${esc(ty)}</span>${demoTag} <strong>${scrubTextHtml(String(sum).slice(0, 120))}</strong>
              <div class="muted small">${esc(src)} · ${own} · conf ${esc(conf)} · ${c.claim_id ? prettyRef(c.claim_id) : ""}</div></li>`;
          })
          .join("");
      }
    }
    if (note) {
      note.textContent = simple
        ? "Same data as technical view — shown in plain language."
        : (led.note || "") + (eng?.adequacy_note ? " · " + eng.adequacy_note : "");
    }
  } catch (e) {
    if (stats) stats.innerHTML = `<span class="pill down">ledger: ${esc(e.message || e)}</span>`;
    if (list) list.innerHTML = `<li class="muted">Failed to load intent ledger</li>`;
  }
}

async function captureIntentClaim() {
  const tenant = syncTenantFields(activeTenant());
  const simple = isSimpleMode();
  const text = window.prompt(
    simple
      ? "What's the focus? (e.g. “stuck on security review” or “aiming to ship neon export”)"
      : "State a purpose claim (e.g. \"blocked on security review\" or \"ready to ship neon export\").\nClassified in-house — no inventing work items.",
    ""
  );
  if (text == null || !String(text).trim()) return;
  const owner =
    $("ck-profile-subject")?.value?.trim() ||
    window.prompt(
      simple ? "Whose focus is this? (name or github login)" : "Owner subject (github login / gu_*)",
      "neeljoshi18"
    ) ||
    "";
  try {
    const body = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/intent/claims`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        text: String(text).trim(),
        owner_subject: owner || undefined,
        channel: "champion_ui",
      }),
    });
    const c = body.claim || {};
    if (simple) {
      alert(
        `Saved: ${plainIntentType(c.intent_type)}\n${c.summary || ""}\n\nIt shows under Open focuses and Needs you today.`
      );
    } else {
      alert(
        `Captured ${c.intent_type || "?"} (${Math.round((c.confidence || 0) * 100)}% conf)\n${c.summary || ""}\nid: ${c.claim_id || ""}`
      );
    }
    await loadIntentLedger();
    if (typeof refreshCockpit === "function") await refreshCockpit();
  } catch (e) {
    alert("Capture failed: " + (e.message || e));
  }
}

if ($("ck-profile-load")) {
  $("ck-profile-load").addEventListener("click", () => loadPersonProfile({ intoLab: false }));
}
if ($("ck-profile-follow")) {
  $("ck-profile-follow").addEventListener("click", () => loadFollowThroughOnly());
}
if ($("lab-profile-load")) {
  $("lab-profile-load").addEventListener("click", () => loadPersonProfile({ intoLab: true }));
}
if ($("ck-intent-refresh")) {
  $("ck-intent-refresh").addEventListener("click", () => loadIntentLedger());
}
if ($("ck-intent-capture")) {
  $("ck-intent-capture").addEventListener("click", () => captureIntentClaim());
}
if ($("ck-intent-demo")) {
  $("ck-intent-demo").addEventListener("change", () => loadIntentLedger());
}

async function loadPlainInsights() {
  const tenant = syncTenantFields(activeTenant());
  const head = $("ck-insights-headline");
  const act = $("ck-insights-act");
  const watch = $("ck-insights-watch");
  const wins = $("ck-insights-wins");
  const how = $("ck-insights-how");
  try {
    const ins = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/intent/insights`
    );
    if (head) head.textContent = softenInsightCopy(ins.headline || "—");
    const renderList = (el, items, empty) => {
      if (!el) return;
      const arr = items || [];
      if (!arr.length) {
        el.innerHTML = `<p class="muted small">${esc(empty)}</p>`;
        return;
      }
      el.innerHTML = arr
        .map((it) =>
          amRow({
            title: scrubTextHtml(softenInsightCopy(it.text || "")),
            meta: scrubTextHtml(softenInsightCopy(it.action || "")),
            tone: it.priority === "high" ? "urgent" : it.priority === "info" ? "ok" : "soon",
          })
        )
        .join("");
    };
    renderList(act, ins.act_on_today, "Nothing urgent right now.");
    renderList(watch, ins.worth_watching, "Nothing on the watch list.");
    renderList(wins, ins.good_news, "No trajectory notes yet.");
    if (how && ins.how_we_read_signals) {
      how.textContent = ins.how_we_read_signals.simple || "";
    }
  } catch (e) {
    if (head) head.textContent = "Could not load insights: " + (e.message || e);
  }
}

let __cmtMode = "team";
async function loadCommitments(mode) {
  if (mode) __cmtMode = mode;
  const tenant = syncTenantFields(activeTenant());
  const subject = $("ck-profile-subject")?.value?.trim() || "neeljoshi18";
  let q = "status=open&limit=40";
  if (__cmtMode === "mine") q += `&i_owe=${encodeURIComponent(subject)}`;
  if (__cmtMode === "owed") q += `&owed_to=${encodeURIComponent(subject)}`;
  const list = $("ck-cmt-list");
  const stats = $("ck-cmt-stats");
  const note = $("ck-cmt-note");
  try {
    const body = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/commitments?${q}`
    );
    const simple = isSimpleMode();
    if (stats) {
      const modeLabel =
        __cmtMode === "mine" ? "I owe" : __cmtMode === "owed" ? "Owed to me" : "Whole team";
      stats.innerHTML = simple
        ? [
            `<span class="pill up">${esc(String(body.open_count ?? 0))} open promises</span>`,
            `<span class="pill mid">${esc(modeLabel)}</span>`,
          ].join(" ")
        : [
            `<span class="pill up">open: ${esc(String(body.open_count ?? 0))}</span>`,
            `<span class="pill mid">shown: ${esc(String(body.count ?? 0))}</span>`,
            `<span class="pill mid">view: ${esc(__cmtMode)}</span>`,
          ].join(" ");
    }
    const rows = body.commitments || [];
    if (list) {
      if (!rows.length) {
        list.innerHTML = simple
          ? `<li class="muted">No open promises. When someone says “I'll…”, it shows up here — or add one.</li>`
          : `<li class="muted">No open commitments. Capture from Slack (“I'll…”) or Add commitment.</li>`;
      } else {
        list.innerHTML = rows
          .map((c) => {
            const id = c.id || "";
            const who = c.promiser_label || c.promiser || "";
            const to = c.promisee_label || c.promisee || "";
            const lin = c.linear_url
              ? `<a class="cmt-linear" href="${esc(c.linear_url)}" target="_blank" rel="noopener">Open in Linear ↗</a>`
              : "";
            return `<li class="ux-card ux-cmt-card">
              ${amRow({
                title: esc(c.headline || c.text || ""),
                meta:
                  `${humanOwnerHtml(who)}${to ? " → " + humanOwnerHtml(to) : ""}` +
                  (simple ? "" : ` · ${esc(c.source || "")} ${prettyRef(id)}`) +
                  (lin ? ` · ${lin}` : ""),
                tone: "open",
                actionsHtml: `<button type="button" class="primary cmt-done" data-id="${esc(id)}">Done</button>
                  <button type="button" class="ghost cmt-dismiss" data-id="${esc(id)}">Not doing</button>
                  <button type="button" class="ghost cmt-linear-btn" data-id="${esc(id)}">Send to Linear</button>`,
              })}
            </li>`;
          })
          .join("");
        list.querySelectorAll(".cmt-done").forEach((btn) => {
          btn.addEventListener("click", async () => {
            const id = btn.getAttribute("data-id");
            try {
              await jfetch(
                `/v3/tenants/${encodeURIComponent(tenant)}/commitments/${encodeURIComponent(id)}/done`,
                { method: "POST", body: "{}" }
              );
              await loadCommitments();
              await loadPlainInsights();
            } catch (e) {
              alert(e.message || e);
            }
          });
        });
        list.querySelectorAll(".cmt-dismiss").forEach((btn) => {
          btn.addEventListener("click", async () => {
            const id = btn.getAttribute("data-id");
            try {
              await jfetch(
                `/v3/tenants/${encodeURIComponent(tenant)}/commitments/${encodeURIComponent(id)}/dismiss`,
                { method: "POST", body: "{}" }
              );
              await loadCommitments();
              await loadPlainInsights();
            } catch (e) {
              alert(e.message || e);
            }
          });
        });
        list.querySelectorAll(".cmt-linear-btn").forEach((btn) => {
          btn.addEventListener("click", async () => {
            const id = btn.getAttribute("data-id");
            try {
              const r = await jfetch(
                `/v3/tenants/${encodeURIComponent(tenant)}/commitments/${encodeURIComponent(id)}/export_linear`,
                { method: "POST", body: "{}" }
              );
              if (r.linear_url) {
                window.open(r.linear_url, "_blank", "noopener");
              } else {
                alert(r.note || JSON.stringify(r));
              }
              await loadCommitments();
            } catch (e) {
              alert(
                (e.message || e) +
                  "\n\nOptional: set LINEAR_API_KEY + LINEAR_TEAM_ID on staging. Commitment stays source of truth."
              );
            }
          });
        });
      }
    }
    if (note) note.textContent = body.note || "";
  } catch (e) {
    if (stats) stats.innerHTML = `<span class="pill down">${esc(e.message || e)}</span>`;
  }
}

async function addCommitmentUi() {
  const tenant = syncTenantFields(activeTenant());
  const simple = isSimpleMode();
  const text = window.prompt(
    simple
      ? "What was promised?\ne.g. I'll send the security write-up by Friday"
      : "What was promised? (plain English)\ne.g. I'll send the security write-up by Friday",
    ""
  );
  if (text == null || !String(text).trim()) return;
  const promiser =
    window.prompt(
      simple ? "Who promised? (name or github login)" : "Who promised? (github login / name)",
      $("ck-profile-subject")?.value || "neeljoshi18"
    ) || "unknown";
  const promisee = simple
    ? window.prompt("Who is owed? (optional — leave blank)", "") || undefined
    : undefined;
  try {
    await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/commitments`, {
      method: "POST",
      body: JSON.stringify({
        text: String(text).trim(),
        promiser: String(promiser).trim(),
        promisee: promisee ? String(promisee).trim() : undefined,
        channel: "champion_ui",
      }),
    });
    await loadCommitments();
    await loadPlainInsights();
    if (typeof refreshCockpit === "function") await refreshCockpit();
  } catch (e) {
    alert("Add failed: " + (e.message || e));
  }
}

if ($("ck-insights-refresh")) {
  $("ck-insights-refresh").addEventListener("click", () => loadPlainInsights());
}
if ($("ck-cmt-refresh")) {
  $("ck-cmt-refresh").addEventListener("click", () => loadCommitments());
}
if ($("ck-cmt-add")) {
  $("ck-cmt-add").addEventListener("click", () => addCommitmentUi());
}
if ($("ck-cmt-mine")) {
  $("ck-cmt-mine").addEventListener("click", () => loadCommitments("mine"));
}
if ($("ck-cmt-owed")) {
  $("ck-cmt-owed").addEventListener("click", () => loadCommitments("owed"));
}
if ($("ck-cmt-team")) {
  $("ck-cmt-team").addEventListener("click", () => loadCommitments("team"));
}
if ($("ck-cmt-digest")) {
  $("ck-cmt-digest").addEventListener("click", async () => {
    const tenant = syncTenantFields(activeTenant());
    const pre = $("ck-cmt-digest-preview");
    try {
      const d = await jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/commitments/digest`
      );
      if (pre) {
        pre.classList.remove("hidden");
        pre.textContent = d.text || JSON.stringify(d, null, 2);
      }
    } catch (e) {
      if (pre) {
        pre.classList.remove("hidden");
        pre.textContent = e.message || String(e);
      }
    }
  });
}
if ($("ck-cmt-digest-send")) {
  $("ck-cmt-digest-send").addEventListener("click", async () => {
    const tenant = syncTenantFields(activeTenant());
    const pre = $("ck-cmt-digest-preview");
    try {
      const d = await jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/commitments/digest/send`,
        { method: "POST", body: "{}" }
      );
      if (pre) {
        pre.classList.remove("hidden");
        pre.textContent =
          (d.posted ? "Sent to Slack.\n\n" : "Not posted (check channel env).\n\n") +
          (d.text || "") +
          "\n\n" +
          (d.detail || "");
      }
      alert(d.detail || (d.posted ? "Digest sent" : "Preview only"));
    } catch (e) {
      alert(e.message || e);
    }
  });
}

// UX mode toggle — presentation only (all features always available)
function wireUxModeButtons() {
  document.querySelectorAll(".ux-mode-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => toggleUxMode(e));
  });
  if ($("btn-ux-simple")) {
    $("btn-ux-simple").addEventListener("click", (e) => {
      e.preventDefault();
      setUxMode("simple");
    });
  }
  if ($("btn-ux-technical")) {
    $("btn-ux-technical").addEventListener("click", (e) => {
      e.preventDefault();
      setUxMode("technical");
    });
  }
}
wireUxModeButtons();
// Apply saved mode (re-render after short tick so cockpit can paint)
setUxMode(getUxMode(), { rerender: false });
setTimeout(() => {
  try {
    rerenderActiveViewForUx();
  } catch (_) {}
}, 50);

function applyStatusUxMode() {
  const view = $("view-status");
  if (!view) return;
  // Keep the draft body visible in both modes — it is the page.
  view.querySelectorAll("code").forEach((el) => {
    if (el.closest("#st-text")) return;
    el.classList.toggle("ux-hide-simple", isSimpleMode());
  });
}

function applyInsightsUxMode() {
  const simple = isSimpleMode();
  const view = $("view-insights");
  if (!view) return;
  const h2 = view.querySelector("h2");
  if (h2) h2.textContent = "Team rhythm";
  const intro = view.querySelector("p.muted.small");
  if (intro && intro.closest(".card") === view.querySelector(".card")) {
    intro.textContent = simple
      ? "When the team is active — from real work on the map. Not who is “best.” Times in IST."
      : "When the team is active — from the live map. Times in IST. Not a ranking.";
  }
  const labels = view.querySelectorAll(".stat-label");
  if (labels[0]) labels[0].textContent = "Recent commits";
  if (labels[1]) labels[1].textContent = simple ? "Work links" : "Authored links";
  if (labels[2]) labels[2].textContent = "Busiest hour (IST)";
}

/** Soften People / Work map / Connect chrome for Simple presentation (same features). */
function applyChromeUxMode() {
  const simple = isSimpleMode();
  // People
  const teamView = $("view-team");
  if (teamView) {
    const h = teamView.querySelector("h2");
    if (h) h.textContent = "Your people";
    const p = teamView.querySelector(".card > p.muted.small");
    if (p) {
      p.textContent = simple
        ? "Map at least two people so status updates cover the whole team. Chat ids link delivery — we never read private one-to-ones."
        : "Map at least two people for team updates. Chat ids link delivery only — never private one-to-ones. Maps persist across restarts.";
    }
    const intentsH = Array.from(teamView.querySelectorAll("h2")).find((el) =>
      /intents|focus/i.test(el.textContent || "")
    );
    if (intentsH) intentsH.textContent = "Open focuses";
    const intentsP = intentsH?.parentElement?.querySelector("p.muted.small");
    if (intentsP) {
      intentsP.textContent = simple
        ? "What people are trying to do — stuck, ship, hold — from work titles and labels."
        : "Purpose claims from work titles and labels — same list, with tags in this view.";
    }
  }
  // Work map
  const graphView = $("view-graph");
  if (graphView) {
    const h = graphView.querySelector("h2");
    if (h) h.textContent = "Work map";
    const p = graphView.querySelector(".graph-toolbar p.muted.small");
    if (p) {
      p.textContent = simple
        ? "People connected to the work they touch. Drag, zoom, click a node for the story."
        : "People connected to work and focuses. Drag, zoom, click a node. Recent work only by default.";
    }
  }
  // Connect
  const connView = $("view-connections");
  if (connView) {
    const h = connView.querySelector("h2");
    if (h) h.textContent = simple ? "Get set up" : "Onboarding";
    const p = connView.querySelector(".card > p.muted.small");
    if (p) {
      p.textContent = simple
        ? "Link chat and GitHub, paste the Slack Events URL, map the pod, then status writes itself."
        : "Tenant → Slack → Events URL + channel invite → GitHub → batch notify → first digest. Server checks stack; OAuth needs human secrets.";
    }
  }
  // Today
  const todayView = $("view-today");
  if (todayView) {
    const prefer = todayView.querySelector(".card > p.muted.small");
    if (prefer) {
      prefer.innerHTML = `Prefer <strong>Home</strong> for the full picture. Path: status → My update → Work map.`;
    }
  }
}

function loadDevInsightsView() {
  if (typeof refreshDevInsights === "function") refreshDevInsights();
  applyInsightsUxMode();
}

async function refreshToday() {
  if (typeof refreshPulse === "function") await refreshPulse();
  if (typeof refreshReadiness === "function") await refreshReadiness();
  if (typeof refreshTeamDigestsToday === "function") await refreshTeamDigestsToday().catch(() => {});
}

// Boot: pilot tenant + champion cockpit default (or Connections after OAuth return)
syncTenantFields(PILOT_TENANT);
refreshReadiness();
if ($("conn-refresh-after")) {
  $("conn-refresh-after").addEventListener("click", () => {
    refreshConnectors();
    refreshOnboarding();
    refreshHealth();
  });
}
const landedOnConnect = handleConnectReturn();
if (!landedOnConnect && $("view-cockpit") && !$("view-cockpit").classList.contains("hidden")) {
  refreshCockpit();
}

// Cockpit actions
if ($("ck-refresh")) $("ck-refresh").addEventListener("click", () => refreshCockpit());
if ($("ck-compile")) {
  $("ck-compile").addEventListener("click", async () => {
    const msg = $("ck-msg");
    if (msg) msg.textContent = "Compiling digests…";
    // ensure team-tenant aligned
    syncTenantFields(activeTenant());
    if ($("team-tenant")) $("team-tenant").value = activeTenant();
    await compileTeamDigests();
    await refreshCockpit();
  });
}
if ($("ck-enrich")) {
  $("ck-enrich").addEventListener("click", async () => {
    const msg = $("ck-msg");
    if (msg) msg.textContent = "Enriching story…";
    try {
      await enrichGraphStory();
      await refreshCockpit();
      if (msg) msg.textContent = "Story enriched.";
    } catch (e) {
      if (msg) msg.textContent = "Enrich failed: " + (e.message || e);
    }
  });
}
if ($("ck-graph")) $("ck-graph").addEventListener("click", () => showView("graph"));
if ($("ck-graph-2")) $("ck-graph-2").addEventListener("click", () => showView("graph"));
if ($("ck-team")) $("ck-team").addEventListener("click", () => showView("team"));
if ($("ck-insights")) $("ck-insights").addEventListener("click", () => showView("insights"));
if ($("ck-connect")) $("ck-connect").addEventListener("click", () => showView("connections"));
// Simple visual home actions (same product surface as Technical ops stack)
if ($("viz-refresh")) $("viz-refresh").addEventListener("click", () => refreshCockpit());
if ($("viz-compile")) {
  $("viz-compile").addEventListener("click", async () => {
    const tenant = syncTenantFields(activeTenant());
    try {
      await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team/compile`, {
        method: "POST",
        body: "{}",
      });
      await refreshCockpit();
      alert("Status updates compiled. Open a person card to review.");
    } catch (e) {
      alert(e.message || e);
    }
  });
}
if ($("viz-add-promise")) {
  $("viz-add-promise").addEventListener("click", () => addCommitmentUi());
}
if ($("viz-capture-focus")) {
  $("viz-capture-focus").addEventListener("click", () => captureIntentClaim());
}
if ($("viz-digest-preview")) {
  $("viz-digest-preview").addEventListener("click", async () => {
    const tenant = syncTenantFields(activeTenant());
    const box = $("viz-digest-box");
    try {
      const d = await jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/commitments/digest`
      );
      if (box) {
        box.classList.remove("hidden");
        box.textContent = d.text || "No open commitments for the morning digest.";
      }
    } catch (e) {
      if (box) {
        box.classList.remove("hidden");
        box.textContent = e.message || String(e);
      }
    }
  });
}
document.querySelectorAll(".viz-filter").forEach((btn) => {
  btn.addEventListener("click", () => {
    __vizCmtFilter = btn.getAttribute("data-filter") || "team";
    if (typeof refreshCockpit === "function") refreshCockpit();
  });
});
if ($("viz-people")) $("viz-people").addEventListener("click", () => showView("team"));
if ($("viz-connect")) $("viz-connect").addEventListener("click", () => showView("connections"));
if ($("viz-map")) $("viz-map").addEventListener("click", () => showView("graph"));
if ($("btn-team-bulk")) $("btn-team-bulk").addEventListener("click", () => bulkImportTeam());
if ($("btn-gh-webhook-copy")) {
  $("btn-gh-webhook-copy").addEventListener("click", async () => {
    const t = $("conn-gh-webhook")?.textContent?.trim();
    if (!t || t === "—") {
      await refreshConnectors();
    }
    const url = $("conn-gh-webhook")?.textContent?.trim();
    if (url && url !== "—") {
      try {
        await navigator.clipboard.writeText(url);
        alert("Webhook URL copied");
      } catch {
        prompt("Copy webhook URL:", url);
      }
    }
  });
}
if ($("btn-slack-events-copy")) {
  $("btn-slack-events-copy").addEventListener("click", async () => {
    const t = $("conn-slack-events")?.textContent?.trim();
    if (!t || t === "—") {
      await refreshConnectors();
    }
    const url = $("conn-slack-events")?.textContent?.trim();
    if (url && url !== "—") {
      try {
        await navigator.clipboard.writeText(url);
        alert("Slack Events URL copied");
      } catch {
        prompt("Copy Slack Events URL:", url);
      }
    }
  });
}
if ($("btn-team-add")) {
  $("btn-team-add").addEventListener("click", addTeamMember);
}
if ($("btn-graph-refresh")) {
  $("btn-graph-refresh").addEventListener("click", () => refreshGraph(true));
}
if ($("btn-graph-fit")) {
  $("btn-graph-fit").addEventListener("click", () => {
    fitGraph({ hubsOnly: true, maxScale: 1.3, minScale: 0.65 });
    drawGraph();
  });
}
if ($("btn-graph-reset")) {
  $("btn-graph-reset").addEventListener("click", () => {
    applyHierarchicalSeed(graphState.nodes, graphState.edges);
    scheduleFitAfterSettle();
    drawGraph();
  });
}
async function enrichGraphStory() {
  const tenant = graphTenant();
  const btn = $("btn-graph-story");
  if (btn) {
    btn.disabled = true;
    btn.textContent = "Enriching…";
  }
  try {
    await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/seed/dual_digests`, {
      method: "POST",
      body: "{}",
    }).catch(() => ({}));
    await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/seed/graph_story`, {
      method: "POST",
      body: "{}",
    });
    // Also keep classic intent demo available if user unchecks Hide demo
    graphState.storyTried = true;
    await refreshGraph(true);
  } catch (e) {
    console.warn("graph story enrich failed", e);
    // fallback intent demo
    try {
      await seedIntentDemo();
    } catch (_) {}
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = "Enrich story";
    }
  }
}
if ($("btn-graph-story")) {
  $("btn-graph-story").addEventListener("click", () => enrichGraphStory());
}
if ($("graph-hide-demo")) {
  $("graph-hide-demo").addEventListener("change", () => refreshGraph(true));
}
if ($("graph-show-unapproved")) {
  $("graph-show-unapproved").addEventListener("change", () => refreshGraph(true));
}
if ($("graph-recent-commits")) {
  $("graph-recent-commits").addEventListener("change", () => refreshGraph(true));
}
if ($("graph-live")) {
  $("graph-live").addEventListener("change", () => {
    if ($("view-graph")?.classList.contains("hidden")) return;
    stopGraphLive();
    if ($("graph-live").checked) {
      graphState.liveTimer = setInterval(() => refreshGraph(false), 8000);
    }
  });
}

refreshHealth();
refreshOnboarding();
loadLatest();
refreshPulse();
refreshMetrics();
setInterval(refreshHealth, 10000);
setInterval(refreshOnboarding, 15000);
setInterval(refreshPulse, 30000);
setInterval(refreshMetrics, 20000);


async function refreshDevInsights() {
  const tenant =
    $("team-tenant")?.value?.trim() ||
    $("tenant")?.value?.trim() ||
    "ten_github";
  const msg = $("insights-msg");
  if (msg) msg.textContent = "Loading…";
  try {
    const d = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/insights/dev`
    );
    const act = d.activity || {};
    const g = d.graph || {};
    if ($("ins-commits")) $("ins-commits").textContent = String(g.commit_nodes ?? "—");
    if ($("ins-authored")) $("ins-authored").textContent = String(act.authored_edges ?? "—");
    const hod = act.hour_of_day_ist || act.hour_of_day_utc || {};
    if ($("ins-peak")) {
      const h = hod.peak_hour_ist ?? hod.peak_hour_utc;
      $("ins-peak").textContent =
        h == null ? "—" : `${String(h).padStart(2, "0")}:00 IST`;
    }
    if ($("ins-insight")) $("ins-insight").textContent = softenInsightCopy(act.insight || "");
    if ($("ins-hours")) {
      $("ins-hours").innerHTML = heatBarsHtml(hod.counts || [], hod.labels || []);
    }
    if ($("ins-authors")) {
      const by = act.by_author || {};
      const entries = Object.entries(by).sort((a, b) => b[1] - a[1]);
      $("ins-authors").innerHTML = entries.length
        ? entries
            .map(
              ([k, v]) =>
                `<li><strong>${prettyMaybe(k)}</strong> — ${v} authored</li>`
            )
            .join("")
        : `<li class="muted">No AUTHORED edges yet — wait for commit poller / webhooks.</li>`;
    }
    if ($("ins-days")) {
      const by = act.by_day || {};
      const entries = Object.entries(by).sort((a, b) => a[0].localeCompare(b[0]));
      $("ins-days").innerHTML = entries.length
        ? heatBarsHtml(
            entries.map(([, n]) => n),
            entries.map(([d]) => d.slice(5))
          )
        : `<span class="muted">No day activity yet.</span>`;
    }
    if ($("ins-recent")) {
      const rec = d.recent_commits || [];
      $("ins-recent").innerHTML = rec.length
        ? rec
            .map((c) => {
              const m = (c.message || c.title || "").toString().trim();
              const msg = m && m !== (c.sha7 || "") && !isOpaqueToken(m)
                ? esc(m.slice(0, 100))
                : `<span class="muted">no message</span>`;
              return `<li>${prettyRef(c.sha7 || c.resource_id || c.id || "")} ${msg}</li>`;
            })
            .join("")
        : `<li class="muted">No commit nodes on graph yet.</li>`;
    }
    if (msg) {
      const people = (d.digests && d.digests.people_with_content) || 0;
      const twins = (d.digests && d.digests.person_twins) || 0;
      msg.textContent = `${g.nodes || 0} items on the map · ${people} of ${twins} people have a status story`;
    }
  } catch (e) {
    if (msg) msg.textContent = "Insights failed: " + (e.message || e);
  }
}

document.addEventListener("click", async (ev) => {
  const btn = ev.target.closest?.(".id-eye");
  if (!btn) return;
  ev.preventDefault();
  ev.stopPropagation();
  const raw = btn.getAttribute("data-id") || "";
  if (!raw) return;
  try {
    await navigator.clipboard.writeText(raw);
    btn.classList.add("copied");
    const tip = btn.querySelector(".id-eye-tip");
    const prev = tip ? tip.textContent : "";
    if (tip) tip.textContent = "Copied";
    setTimeout(() => {
      btn.classList.remove("copied");
      if (tip) tip.textContent = prev || raw;
    }, 900);
  } catch (_) {
    prompt("Identifier:", raw);
  }
});
