const $ = (id) => document.getElementById(id);

/** Single product tenant for pilot sales path (not lab ten_demo). */
const PILOT_TENANT = "ten_github";

let state = {
  tenant: PILOT_TENANT,
  draftId: null,
  ledgerId: null,
  latest: null,
  status: null,
};

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
  if (s >= 3600) return `${Math.round(s / 3600)}h`;
  if (s >= 60) return `${Math.round(s / 60)}m`;
  return `${s}s`;
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
  const titles = {
    cockpit: ["Champion cockpit", "Pod pulse · digests · conflicts · heat · tomorrow focus"],
    today: ["Today", "Org pulse — lighter view; use Cockpit for full operator console"],
    status: ["My status", "Approve / edit / don't send · change-only Slack"],
    team: ["Team", "Map eng pod · bulk import · compile digests"],
    graph: ["Graph", "Live context map — people, work, intents, edges"],
    connections: ["Connections", "Services and on-demand test status"],
    settings: ["Settings", "Cadence, metrics, product boundaries"],
    insights: ["Dev insights", "Activity heat · commits · when you ship — data is currency"],
    lab: ["Lab", "Engineer console and raw JSON"],
  };
  if (name === "cockpit") {
    refreshCockpit();
  }
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
  }
  if (name === "status") {
    loadLatest();
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
  const t = titles[name] || ["AI Manager", ""];
  $("view-title").textContent = t[0];
  $("view-sub").textContent = t[1];
}

async function refreshHealth() {
  try {
    const h = await jfetch("/v3/demo/status");
    state.status = h;
    $("conn-pills").innerHTML = [
      pill("V3", true),
      pill("V1", h.v1),
      pill("V2", h.v2),
      pill("egress", h.egress),
    ].join("");
    const stackOk = h.v1 && h.v2;
    $("stat-stack").textContent = stackOk ? "Live" : "Partial";
    $("stat-stack-detail").textContent = stackOk
      ? "V1 ingest + V2 graph reachable"
      : "Start stack with ./scripts/dev_up.sh or docker compose -f deploy/docker-compose.app.yml up -d";
    $("stat-notify").textContent = fmtSecs(h.notify_interval_secs);
    $("stat-window").textContent = fmtSecs(h.status_window_secs);
    $("conn-detail").textContent = `Slack: ${h.slack_mode || "—"} · runtime ${h.mode || "—"} · notify ${h.notify_policy || "v1"}`;
    $("nav-mode").textContent = `${h.mode || "?"} · ${h.slack_mode || "slack?"}`;
    $("cfg-window").textContent = String(h.status_window_secs ?? "—");
    $("cfg-notify").textContent = String(h.notify_interval_secs ?? "—");
    $("cfg-compile").textContent = String(h.compile_interval_secs ?? "—");
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
      const durability =
        " Persistence: V1 events + ACL identity, V2 graph snapshot, V3 twins on disk (survive restarts).";
      $("conn-graph-detail").textContent = (h.graph_message || "") + durability;
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
  return `<div class="muted small">evidence: ${list.map((r) => esc(r)).join(" · ")}</div>`;
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
  $("st-ids").textContent = `ledger=${state.ledgerId || "—"}  draft=${state.draftId || "—"}`;
  $("st-text").textContent = payload.draft?.draft_text || "(no text)";

  const items = payload.ledger?.items || [];
  const blockers = payload.ledger?.open_blockers || [];
  $("st-items").innerHTML = "";
  for (const it of items) {
    const li = document.createElement("li");
    li.innerHTML =
      `<strong>[${esc(it.confidence)}]</strong> ${esc(it.summary)}` +
      (it.resource_id ? ` <span class="muted small">(${esc(it.resource_id)})</span>` : "") +
      renderEvidenceLine(it.evidence_refs);
    $("st-items").appendChild(li);
  }
  for (const b of blockers) {
    const li = document.createElement("li");
    li.innerHTML =
      `<strong>[blocker]</strong> ${esc(b.summary)}` + renderEvidenceLine(b.evidence_refs);
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
    <pre class="box">${esc(payload.draft?.draft_text || "(no text)")}</pre>
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
    alert("No draft yet — open a person from Today/Team digests, or Compile all digests first");
    return;
  }
  const base = `/v3/tenants/${encodeURIComponent(state.tenant)}/drafts/${encodeURIComponent(state.draftId)}`;
  const labels = { publish: "Approve", veto: "Don't send", edit: "Edit" };
  const label = labels[kind] || kind;
  try {
    if (kind === "edit") {
      const text = prompt("Edited status text:", $("st-text").textContent);
      if (text == null) return;
      await jfetch(base + "/edit", { method: "POST", body: JSON.stringify({ text }) });
    } else {
      await jfetch(base + "/" + kind, { method: "POST", body: "{}" });
    }
    // Reload same draft after action
    await openDraftById(state.tenant, state.draftId, state.ledgerId);
    if (kind === "publish") {
      const st = state.latest?.draft?.status || "";
      if (st === "published" || st === "Published") {
        /* ok */
      }
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
    el.innerHTML = [
      `<span class="pill ${soft || a2 ? "up" : "mid"}">sales: ${soft || a2 ? "ready" : "solo ok"}</span>`,
      `<span class="pill ${multi ? "up" : "mid"}">multi-person: ${multi ? "yes" : "no"}</span>`,
      `<span class="pill ${content >= 2 ? "up" : content >= 1 ? "mid" : "down"}">digests with content: ${content}</span>`,
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
        `/v3/tenants/${encodeURIComponent(tenant)}/pulse?refresh=1`
      ).catch(() => ({ conflicts: { cards: [] }, intents: {} })),
      jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/insights/dev`).catch(() => null),
      jfetch(
        `/v3/tenants/${encodeURIComponent(tenant)}/graph?node_limit=200&edge_limit=400&include_demo=false`
      ).catch(() => null),
    ]);

    // Readiness
    const soft = ready.soft_outreach_ready === true;
    const multi = ready.multi_person_ready === true || team.multi_person_ready === true;
    const content = ready.content_people ?? 0;
    if ($("ck-readiness")) {
      $("ck-readiness").innerHTML = [
        `<span class="pill ${soft ? "up" : "mid"}">soft outreach: ${soft ? "ready" : "solo ok"}</span>`,
        `<span class="pill ${multi ? "up" : "mid"}">multi-person: ${multi ? "yes" : "need ≥2"}</span>`,
        `<span class="pill ${content >= 2 ? "up" : content >= 1 ? "mid" : "down"}">content digests: ${content}</span>`,
        `<span class="pill mid">${esc((ready.note || ready.error || "").toString().slice(0, 100))}</span>`,
      ].join(" ");
    }

    const members = team.members || [];
    const mapped = members.filter((m) => m.slack_mapped).length;
    const withContent = members.filter((m) => m.last_digest?.has_content).length;
    const cards = pulse.conflicts?.cards || [];
    const confCount = pulse.conflicts?.count ?? cards.length;

    if ($("ck-stat-mapped")) $("ck-stat-mapped").textContent = String(mapped);
    if ($("ck-stat-mapped-d"))
      $("ck-stat-mapped-d").textContent = `${members.length} twins · ${team.unique_slack_users ?? mapped} unique chat`;
    if ($("ck-stat-content")) $("ck-stat-content").textContent = String(withContent);
    if ($("ck-stat-content-d"))
      $("ck-stat-content-d").textContent = `${members.filter((m) => m.last_digest).length} with any draft`;
    if ($("ck-stat-conflicts")) $("ck-stat-conflicts").textContent = String(confCount);
    if ($("ck-stat-conflicts-d"))
      $("ck-stat-conflicts-d").textContent =
        confCount > 0 ? "shared-work conflicts live" : "no open work conflicts";

    // Pod roster
    const pod = $("ck-pod");
    if (pod) {
      if (!members.length) {
        pod.innerHTML = `<li class="muted">No pod members — open <strong>Team</strong> and map people (or bulk import).</li>`;
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
                <strong>${esc(m.display_name || m.subject_id)}</strong>
                ${m.slack_mapped ? "" : " · <span class='muted'>unmapped chat</span>"}
              </button>
              <div class="muted small">${esc(dig)}${d?.preview ? " · " + esc(d.preview.slice(0, 72)) : ""}</div>
            </li>`;
          })
          .join("");
        pod.querySelectorAll(".dig-open").forEach((btn) => {
          btn.addEventListener("click", async () => {
            const did = btn.getAttribute("data-draft");
            const lid = btn.getAttribute("data-ledger");
            if (!did) {
              alert("No draft yet — Compile digests first");
              return;
            }
            const ok = await openDraftById(tenant, did, lid);
            if (ok) showView("status");
          });
        });
      }
    }

    // Conflicts
    const confEl = $("ck-conflicts");
    if (confEl) {
      if (!cards.length) {
        confEl.innerHTML = `<p class="muted">No open live conflicts. Enrich story or ship real dual-owner PRs to surface SHIP/FREEZE.</p>`;
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
      intentUl.innerHTML = sample.length
        ? sample
            .slice(0, 12)
            .map((n) => {
              const ty = n.intent_type || n.type || "Intent";
              return `<li><strong>${esc(ty)}</strong> ${esc(n.label || n.title || n.id || "")}</li>`;
            })
            .join("")
        : `<li class="muted">No live intents in sample</li>`;
    }

    // Heat
    if (ins && ins.activity) {
      const act = ins.activity;
      if ($("ck-heat-insight")) $("ck-heat-insight").textContent = act.insight || "";
      const hod = act.hour_of_day_utc || {};
      const counts = hod.counts || [];
      const labels = hod.labels || [];
      if ($("ck-heat-hours")) {
        let lines = [];
        for (let i = 0; i < counts.length; i++) {
          const n = counts[i] || 0;
          if (!n) continue;
          const bar = "█".repeat(Math.min(28, n));
          lines.push(`${labels[i] || i}: ${bar} ${n}`);
        }
        $("ck-heat-hours").textContent = lines.join("\n") || "No heat yet.";
      }
      if ($("ck-heat-authors")) {
        const by = act.by_author || {};
        const top = Object.entries(by)
          .sort((a, b) => b[1] - a[1])
          .slice(0, 6)
          .map(([k, v]) => `${k}: ${v} authored`)
          .join(" · ");
        $("ck-heat-authors").textContent = top
          ? `Authored volume (context, not rank): ${top}`
          : "";
      }
    } else if ($("ck-heat-insight")) {
      $("ck-heat-insight").textContent = "Heat unavailable";
    }

    // Graph stats
    if ($("ck-graph-stats") && graph) {
      $("ck-graph-stats").textContent = JSON.stringify(
        {
          nodes: (graph.nodes || []).length,
          edges: (graph.edges || []).length,
          by_type: graph.by_type || {},
          edge_by_type: graph.edge_by_type || {},
        },
        null,
        2
      );
    }

    // Tomorrow focus — suggestions from conflicts, intents, digests + persisted pins
    const tomorrow = [];
    for (const c of cards.slice(0, 5)) {
      tomorrow.push({
        kind: "conflict",
        text: `${c.severity || c.kind}: ${c.summary || c.kind}`,
        why: "Resolve shared-work conflict before next standup",
        pinned: false,
      });
    }
    for (const n of (pulse.intents?.sample || []).slice(0, 5)) {
      const ty = n.intent_type || "Intent";
      if (ty === "BLOCKED" || ty === "FREEZE" || ty === "SHIP") {
        tomorrow.push({
          kind: "intent",
          text: `${ty}: ${n.label || n.title || ""}`,
          why: "Intent needs champion attention",
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
                `<li><span class="pill ${t.pinned ? "up" : "mid"}">${esc(t.kind)}${t.pinned ? " · pinned" : ""}</span> <strong>${esc(t.text)}</strong><div class="muted small">${esc(t.why)}</div></li>`
            )
            .join("")
        : `<li class="muted">No suggestions yet — compile digests or enrich story so open work appears.</li>`;
    }
    if ($("ck-tomorrow-note")) {
      const nPin = pinnedItems.length;
      $("ck-tomorrow-note").textContent = nPin
        ? `${nPin} pinned assignment(s) persisted for this tenant · suggestions merge above.`
        : "Pin the board to persist champion focus across reloads (tenant state).";
    }

    if (msg) {
      msg.textContent = `Updated ${new Date().toISOString().slice(11, 19)} UTC · tenant ${tenant}`;
    }
  } catch (e) {
    if (msg) msg.textContent = "Cockpit failed: " + (e.message || e);
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
      li.innerHTML = `<span class="mark">${step.done ? "✓" : "○"}</span> <strong>${step.title}</strong> — <span class="muted">${step.detail || ""}</span>`;
      el.appendChild(li);
    }
    if ($("onboard-note")) {
      $("onboard-note").textContent = o.note || "";
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
      // Still open install URL if present (e.g. GH slug without full env)
      if (body.install_url) window.open(body.install_url, "_blank", "noopener");
      return;
    }
    const url = body.authorize_url || body.install_url;
    if (!url) {
      alert(JSON.stringify(body, null, 2));
      return;
    }
    // Pre-flight: what will happen
    if (kind === "slack") {
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Connect Slack</strong> — Slack will ask to install the AI Manager bot on your workspace. ` +
          `After you click Allow, you should see “Slack connected”. Digests use that bot token (vault only). ` +
          `If DMs fail after first connect, restart egress once so it reloads secrets.`;
      }
      const ok = confirm(
        "Connect Slack\n\n" +
          "1. Slack will open — install the bot on your workspace\n" +
          "2. Approve chat:write / im:write\n" +
          "3. You’ll land on a “Slack connected” page\n" +
          "4. Come back here and Refresh status\n\n" +
          "Continue to Slack?"
      );
      if (!ok) return;
    } else if (kind === "github") {
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Install GitHub App</strong> — pick the org/repos that should feed status. ` +
          `Webhooks hit <code>${esc(body.webhook_url || "…/webhooks/github")}</code>. ` +
          `Graph fills via V1→bridge→V2 (needs Graph healthy).`;
      }
      const ok = confirm(
        "Install GitHub App\n\n" +
          "1. GitHub will open the App install page\n" +
          "2. Choose org + repositories for status\n" +
          "3. Webhooks post to status.neel.world automatically\n" +
          "4. Return here — open Graph / Cockpit after a minute\n\n" +
          "Continue to GitHub?"
      );
      if (!ok) return;
    } else if (kind === "teams") {
      if (guideEl) {
        guideEl.innerHTML =
          `<strong>Connect Teams</strong> — Azure Bot + Adaptive Cards. Needs TEAMS_APP_ID + vault TEAMS_BOT_TOKEN. ` +
          `Messaging endpoint: <code>${esc(body.messaging_endpoint || "")}</code>`;
      }
    }
    const win = window.open(url, "_blank", "noopener");
    if (!win) {
      alert(
        "Popup blocked. Allow popups for status.neel.world, or open this URL manually:\n\n" +
          url
      );
      if (guideEl) {
        guideEl.innerHTML += ` <a href="${esc(url)}" target="_blank" rel="noopener">Open install link</a>`;
      }
    } else if (guideEl && kind === "slack") {
      guideEl.innerHTML +=
        ` <button type="button" class="ghost" id="conn-refresh-after">I finished — refresh status</button>`;
      $("conn-refresh-after")?.addEventListener("click", () => refreshConnectors());
    }
  } catch (e) {
    alert("OAuth start failed: " + (e.message || e));
  }
}

/** Refresh Connectors panel + oauth pills (Connections). */
async function refreshConnectors() {
  const statusEl = $("conn-oauth-status");
  try {
    const o = await jfetch("/v3/oauth/status");
    const slack = o.slack || {};
    const gh = o.github || {};
    const teams = o.teams || {};
    const teamsPill =
      teams.status === "ready"
        ? "up"
        : teams.status === "configured" || teams.app_id_present
          ? "mid"
          : "mid";
    if (statusEl) {
      statusEl.innerHTML = [
        `<span class="pill ${slack.oauth_credentials ? "up" : "mid"}">Slack OAuth: ${slack.oauth_credentials ? "ready" : "manual vault"}</span>`,
        `<span class="pill ${gh.app_env_present ? "up" : "mid"}">GitHub App: ${gh.app_env_present ? "ready" : "set slug/id"}</span>`,
        `<span class="pill ${teamsPill}">Teams: ${esc(teams.status || "manual")}</span>`,
        `<span class="pill mid">adapter: ${esc(o.delivery_adapter || "slack")}</span>`,
      ].join(" ");
    }
    if ($("conn-github")) {
      $("conn-github").textContent = gh.note || $("conn-github").textContent;
    }
    if ($("conn-gh-webhook") && gh.webhook_url) {
      $("conn-gh-webhook").textContent = gh.webhook_url;
    }
    if ($("conn-slack")) {
      $("conn-slack").textContent =
        (slack.note || "Outbound digests via egress vault.") +
        (slack.egress_mode ? ` Mode: ${slack.egress_mode}.` : "");
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
    // Checklist visual (ready = credentials present; not full end-to-end proof)
    const mark = (id, ok, text) => {
      const el = $(id);
      if (!el) return;
      el.textContent = text;
      el.style.color = ok ? "var(--up, #0a7)" : "";
      el.style.fontWeight = ok ? "600" : "";
    };
    mark(
      "conn-step-slack",
      !!slack.oauth_credentials,
      slack.oauth_credentials
        ? "Connect Slack — OAuth ready (button opens Slack install)"
        : "Connect Slack — set SLACK_CLIENT_ID/SECRET or paste vault token"
    );
    mark(
      "conn-step-gh",
      !!gh.app_env_present,
      gh.app_env_present
        ? "Install GitHub App — ready (button opens App install)"
        : "Install GitHub App — set GITHUB_APP_SLUG / ID"
    );
    mark(
      "conn-step-map",
      true,
      "Map pod under Team (Slack user ids) — bulk import available"
    );
    mark(
      "conn-step-graph",
      true,
      "Graph + digests — needs healthy V2 + bridge after GitHub install"
    );
  } catch (e) {
    if (statusEl) {
      statusEl.innerHTML = `<span class="pill mid">install status: ${esc(e.message || "n/a")}</span>`;
    }
  }
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
      `<span class="pill ${team.multi_person_ready ? "up" : "mid"}">multi-person: ${team.multi_person_ready ? "ready" : "need ≥2"}</span>` +
      `<span class="muted small" style="margin-left:0.5rem;">Click a person → My status (Approve / Don't send)</span>` +
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
            dig = `<strong>${esc(d.status_label || d.status)}</strong> · ${content} · ${d.dm_sent ? "DM sent" : "no DM"} · <span class="muted small">${esc((d.preview || "").slice(0, 80))}</span>`;
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
      `/v3/tenants/${encodeURIComponent(tenant)}/pulse?refresh=1`
    );
    const cards = pulse.conflicts?.cards || [];
    const count = pulse.conflicts?.count ?? cards.length;
    const demoCount = pulse.conflicts?.demo_count ?? 0;
    const multi = pulse.team?.multi_person_ready;
    if (el) {
      if (!count) {
        const demoNote =
          demoCount > 0
            ? ` <span class="muted small">(${demoCount} intent-demo seed card(s) hidden — use <strong>Load intent demo</strong> then uncheck Graph “Hide demo” if you need them)</span>`
            : "";
        el.innerHTML = `<p class="muted">No open <em>live</em> conflicts for <code>${esc(tenant)}</code>. Multi-person ready: <strong>${multi ? "yes" : "no"}</strong> (${pulse.team?.unique_slack_users ?? pulse.team?.slack_mapped ?? 0} unique Slack).${demoNote}</p>`;
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
                `<li><strong>[${esc(c.severity || c.kind)}]</strong> ${esc(c.summary)} <span class="muted small">${esc(c.kind)}</span></li>`
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
            return `<li><strong>[${esc(ty)}]</strong> ${esc(n.display_name || n.node_id)}</li>`;
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
  const when = (d.updated_at || "").toString().replace("T", " ").slice(0, 16);
  return `<span class="pill ${d.has_content ? "up" : "mid"}">${esc(st)} · ${content}</span> <span class="muted small">${esc(dm)}${when ? " · " + esc(when) : ""}</span>`;
}

async function refreshTeam() {
  const tenant = $("team-tenant")?.value?.trim() || "ten_github";
  const body = $("team-body");
  const ready = $("team-ready");
  try {
    const team = await jfetch(`/v3/tenants/${encodeURIComponent(tenant)}/team`);
    if (ready) {
      const withDigest = (team.members || []).filter((m) => m.last_digest).length;
      ready.innerHTML = [
        `<span class="pill ${team.multi_person_ready ? "up" : "mid"}">multi-person: ${team.multi_person_ready ? "ready" : "need ≥2 Slack maps"}</span>`,
        `<span class="pill mid">${team.slack_mapped_count ?? 0} mapped / ${team.person_count ?? 0} members</span>`,
        `<span class="pill mid">${withDigest} with digests</span>`,
      ].join("");
    }
    if (body) {
      const members = team.members || [];
      if (!members.length) {
        body.innerHTML = `<tr><td colspan="5" class="muted">No members yet — add two humans below.</td></tr>`;
      } else {
        body.innerHTML = members
          .map((m) => {
            const aliases = Array.isArray(m.provider_aliases)
              ? m.provider_aliases.join(", ")
              : "";
            const sub = aliases
              ? `${esc(m.subject_id)} <span class="muted">(${esc(aliases)})</span>`
              : esc(m.subject_id);
            return `<tr>
              <td>${esc(m.display_name || "—")}</td>
              <td><code class="small">${sub}</code></td>
              <td><code class="small">${esc(m.slack_user_id || "—")}</code></td>
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
    btn.textContent = "Compiling…";
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
      btn.textContent = "Compile all digests";
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
    const data = await jfetch(
      `/v3/tenants/${encodeURIComponent(tenant)}/graph?node_limit=600&edge_limit=1500&include_demo=${includeDemo ? "true" : "false"}`
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
    if (msg && msg !== sha) return msg.slice(0, 36);
    return sha || n.id;
  }
  if (type === "Intent") {
    return n.intent_type || n.label || n.id;
  }
  if (type === "PullRequest") {
    return (n.title || n.label || "PR").toString().slice(0, 40);
  }
  return n.label || n.id;
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
  const commits = nodes.filter((n) => n.type === "Commit" && n.visual !== false);
  const other = nodes.filter(
    (n) =>
      n.visual !== false &&
      !["Person", "Repo", "PullRequest", "Issue", "Ticket", "Intent", "Commit"].includes(n.type)
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
      `<span class="pill mid">as_of ${esc((data.as_of || "").replace("T", " ").slice(0, 19))}</span>`,
    ]
      .filter(Boolean)
      .join("");
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
    legend.innerHTML = [
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
              `<li><code class="small">${esc(e.type)}</code> <span class="muted">${esc(shortId(e.from))} → ${esc(shortId(e.to))}</span></li>`
          )
          .join("")
      : `<li class="muted">No edges yet</li>`;
  }
  const counts = $("graph-type-counts");
  if (counts) {
    counts.textContent = JSON.stringify(
      {
        nodes: data.by_type || {},
        edges: data.edge_by_type || {},
      },
      null,
      2
    );
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
  el.innerHTML = `
    <div class="meta-row">
      <span class="graph-node-chip">${esc(n.type)}</span>
      ${intent ? `<span class="graph-node-chip">${esc(intent)}</span>` : ""}
      ${n.meta?.from_team_map ? `<span class="graph-node-chip">team map</span>` : ""}
    </div>
    <p style="margin:0.6rem 0 0.2rem;font-weight:600;">${esc(n.label)}</p>
    <p class="muted small" style="margin:0;word-break:break-all;"><code>${esc(n.id)}</code></p>
    ${n.meta?.resource_id ? `<p class="muted small">resource: <code>${esc(n.meta.resource_id)}</code></p>` : ""}
    <p class="muted small" style="margin-top:0.75rem;">${linked.length} edge(s) · ${uniq.length} neighbor(s)</p>
    <ul class="item-list">
      ${linked
        .slice(0, 12)
        .map((e) => {
          const other = e.from === n.id ? e.to : e.from;
          const dir = e.from === n.id ? "→" : "←";
          return `<li><code class="small">${esc(e.type)}</code> ${dir} ${esc(shortId(other))}</li>`;
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
      ? `rgba(17,17,17,${alpha})`
      : isClaim
        ? `rgba(82,82,82,${alpha})`
        : `rgba(163,163,163,${alpha})`;
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
      ctx.fillStyle = inSel || isBlock ? "#404040" : "#a3a3a3";
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
      ctx.fillStyle = "#111";
      ctx.textAlign = "center";
      const maxLen = n.type === "Commit" ? 28 : n.type === "Person" ? 20 : 26;
      ctx.fillText(truncateLabel(n.label, maxLen), n.x, n.y + n.r + 14 / graphState.scale);
      if (n.type === "Intent" && n.meta?.intent_type) {
        ctx.fillStyle = "#737373";
        ctx.font = `${9 / graphState.scale}px ui-monospace, monospace`;
        ctx.fillText(n.meta.intent_type, n.x, n.y + n.r + 24 / graphState.scale);
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
      ? "V2 graph-api is down — autoheal restarting; map will refill"
      : "No graph nodes yet — bridge re-projecting ingested V1 events";
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
    ctx.strokeStyle = "#111";
    ctx.lineWidth = 2 / graphState.scale;
    ctx.stroke();
  }
  ctx.lineWidth = 1.5 / graphState.scale;
  ctx.strokeStyle = "#111";
  ctx.fillStyle = "#fff";

  if (n.type === "Person") {
    ctx.beginPath();
    ctx.arc(0, 0, r, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    // head+shoulders glyph
    ctx.beginPath();
    ctx.arc(0, -r * 0.25, r * 0.28, 0, Math.PI * 2);
    ctx.fillStyle = "#111";
    ctx.fill();
    ctx.beginPath();
    ctx.arc(0, r * 0.55, r * 0.55, Math.PI, 0);
    ctx.fill();
  } else if (n.type === "PullRequest") {
    ctx.fillStyle = "#111";
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
    ctx.fillStyle = "#e5e5e5";
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
// Boot: pilot tenant + champion cockpit default
syncTenantFields(PILOT_TENANT);
refreshReadiness();
if ($("view-cockpit") && !$("view-cockpit").classList.contains("hidden")) {
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
    const hod = act.hour_of_day_utc || {};
    if ($("ins-peak")) {
      const h = hod.peak_hour_utc;
      $("ins-peak").textContent =
        h == null ? "—" : `${String(h).padStart(2, "0")}:00`;
    }
    if ($("ins-insight")) $("ins-insight").textContent = act.insight || "";
    if ($("ins-hours")) {
      const counts = hod.counts || [];
      const labels = hod.labels || [];
      let lines = [];
      for (let i = 0; i < counts.length; i++) {
        const n = counts[i] || 0;
        const bar = "█".repeat(Math.min(40, n)) + (n ? ` ${n}` : "");
        lines.push(`${labels[i] || i}: ${bar || "·"}`);
      }
      $("ins-hours").textContent = lines.join("\n");
    }
    if ($("ins-authors")) {
      const by = act.by_author || {};
      const entries = Object.entries(by).sort((a, b) => b[1] - a[1]);
      $("ins-authors").innerHTML = entries.length
        ? entries
            .map(
              ([k, v]) =>
                `<li><strong>${esc(k)}</strong> — ${v} authored</li>`
            )
            .join("")
        : `<li class="muted">No AUTHORED edges yet — wait for commit poller / webhooks.</li>`;
    }
    if ($("ins-days")) {
      const by = act.by_day || {};
      const lines = Object.entries(by)
        .sort((a, b) => a[0].localeCompare(b[0]))
        .map(([d, n]) => `${d}: ${"█".repeat(Math.min(30, n))} ${n}`);
      $("ins-days").textContent = lines.join("\n") || "No day activity yet.";
    }
    if ($("ins-recent")) {
      const rec = d.recent_commits || [];
      $("ins-recent").innerHTML = rec.length
        ? rec
            .map((c) => {
              const sha = esc(c.sha7 || c.resource_id || "?");
              const m = (c.message || c.title || "").toString().trim();
              const msg = m && m !== (c.sha7 || "")
                ? esc(m.slice(0, 100))
                : `<span class="muted">no message</span>`;
              return `<li><code>${sha}</code> ${msg}</li>`;
            })
            .join("")
        : `<li class="muted">No commit nodes on graph yet.</li>`;
    }
    if (msg) {
      msg.textContent = `Graph ${g.nodes || 0} nodes · ${g.edges || 0} edges · digests content ${
        (d.digests && d.digests.people_with_content) || 0
      }/${(d.digests && d.digests.person_twins) || 0}`;
    }
  } catch (e) {
    if (msg) msg.textContent = "Insights failed: " + (e.message || e);
  }
}
