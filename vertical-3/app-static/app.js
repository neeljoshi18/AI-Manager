const $ = (id) => document.getElementById(id);

let state = {
  tenant: "ten_demo",
  draftId: null,
  ledgerId: null,
  latest: null,
  status: null,
};

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
  document.querySelectorAll(".view").forEach((el) => el.classList.add("hidden"));
  document.querySelectorAll(".nav-item").forEach((el) => el.classList.remove("active"));
  const view = $(`view-${name}`);
  if (view) view.classList.remove("hidden");
  const btn = document.querySelector(`.nav-item[data-view="${name}"]`);
  if (btn) btn.classList.add("active");
  const titles = {
    today: ["Today", "Org pulse — what the graph knows right now"],
    status: ["My status", "Approve / edit / don't send · change-only Slack"],
    team: ["Team", "Multi-person Slack map · intents · conflicts"],
    graph: ["Graph", "Live context map — people, work, intents, edges"],
    connections: ["Connections", "Services and on-demand test status"],
    settings: ["Settings", "Cadence, metrics, product boundaries"],
    lab: ["Lab", "Engineer console and raw JSON"],
  };
  if (name === "team") {
    refreshTeam();
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

async function loadLatest() {
  const tenant = $("tenant")?.value?.trim() || "ten_demo";
  try {
    const payload = await jfetch(`/v3/demo/latest?tenant_id=${encodeURIComponent(tenant)}`);
    renderLatest(payload);
  } catch {
    /* no snapshot yet */
  }
}

async function simulate() {
  $("btn-sim").disabled = true;
  $("btn-sim").textContent = "Sending…";
  try {
    const body = {
      tenant_id: $("tenant").value.trim() || "ten_demo",
      global_user_id: $("user").value.trim() || "gu_alice",
      display_name: $("name").value.trim() || "Alice",
      slack_user_id: $("slack_user").value.trim() || "U_DEMO",
      channel_id: $("channel").value.trim() || "C_DEMO",
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
    alert("No draft yet — send a test status first");
    return;
  }
  const base = `/v3/tenants/${encodeURIComponent(state.tenant)}/drafts/${encodeURIComponent(state.draftId)}`;
  try {
    if (kind === "edit") {
      const text = prompt("Edited status text:", $("st-text").textContent);
      if (text == null) return;
      await jfetch(base + "/edit", { method: "POST", body: JSON.stringify({ text }) });
    } else {
      await jfetch(base + "/" + kind, { method: "POST", body: "{}" });
    }
    await loadLatest();
  } catch (e) {
    alert(kind + " failed: " + (e.message || e));
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
  const path = kind === "slack" ? "/v3/oauth/slack/start" : "/v3/oauth/github/start";
  try {
    const res = await fetch(path);
    const body = await res.json().catch(() => ({}));
    if (res.status === 501 || body.error) {
      alert(
        (body.message || "Not configured") +
          "\n\nManual path: " +
          (body.manual_path || body.webhook_path || "deploy/oauth/README.md")
      );
      return;
    }
    const url = body.authorize_url || body.install_url;
    if (url) {
      window.open(url, "_blank", "noopener");
    } else {
      alert(JSON.stringify(body, null, 2));
    }
  } catch (e) {
    alert("OAuth start failed: " + (e.message || e));
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
  const tenant =
    $("team-tenant")?.value?.trim() ||
    $("tenant")?.value?.trim() ||
    "ten_github";
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
      `</div><ul class="item-list">` +
      members
        .map((m) => {
          const d = m.last_digest;
          const dig = d
            ? `<strong>${esc(d.status_label || d.status)}</strong> · ${d.dm_sent ? "DM sent" : d.empty_placeholder ? "empty window" : "no DM"} · <span class="muted small">${esc((d.preview || "").slice(0, 80))}</span>`
            : `<span class="muted">no digest yet</span>`;
          return `<li><strong>${esc(m.display_name || m.subject_id)}</strong> — ${dig}</li>`;
        })
        .join("") +
      `</ul>`;
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
  const when = (d.updated_at || "").toString().replace("T", " ").slice(0, 16);
  return `<span class="pill mid">${esc(st)}</span> <span class="muted small">${esc(dm)}${when ? " · " + esc(when) : ""}</span>`;
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
   Graph live map — force layout, filters, selection, live poll
   ═══════════════════════════════════════════════════════════════ */
const graphState = {
  raw: null,
  nodes: [], // { id, type, label, x, y, vx, vy, r, meta }
  edges: [], // { id, type, from, to }
  filters: {}, // type -> bool
  selected: null,
  sim: null,
  liveTimer: null,
  anim: null,
  drag: null,
  pan: { x: 0, y: 0 },
  scale: 1,
  panning: null,
  lastFetch: 0,
  types: [],
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
    canvas.width = Math.max(320, Math.floor(rect.width * dpr));
    canvas.height = Math.max(360, Math.floor(rect.height * dpr));
    canvas.style.height = Math.max(360, rect.height) + "px";
    drawGraph();
  };
  window.addEventListener("resize", resize);
  resize();

  canvas.addEventListener("wheel", (e) => {
    e.preventDefault();
    const factor = e.deltaY > 0 ? 0.92 : 1.08;
    graphState.scale = Math.min(3.5, Math.max(0.25, graphState.scale * factor));
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
    if (!typeVisible(n.type)) continue;
    const dx = n.x - x;
    const dy = n.y - y;
    if (dx * dx + dy * dy <= (n.r + 4) * (n.r + 4)) return n;
  }
  return null;
}

function typeVisible(t) {
  if (graphState.filters[t] === false) return false;
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
    case "Person": return 16;
    case "PullRequest": return 12;
    case "Issue":
    case "Ticket": return 11;
    case "Intent": return 10;
    case "Repo": return 14;
    default: return 9;
  }
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
      `/v3/tenants/${encodeURIComponent(tenant)}/graph?node_limit=500&edge_limit=1000&include_demo=${includeDemo ? "true" : "false"}`
    );
    graphState.raw = data;
    graphState.lastFetch = Date.now();
    mergeGraphData(data, forceLayout);
    renderGraphChrome(data);
    drawGraph();
  } catch (e) {
    if (statsEl) {
      statsEl.innerHTML = `<span class="pill down">graph load failed: ${esc(e.message || e)}</span>`;
    }
  }
}

function mergeGraphData(data, forceLayout) {
  const prev = new Map(graphState.nodes.map((n) => [n.id, n]));
  const types = new Set();
  // Hide demo seed people + collapse same-label Person nodes (one human = one node)
  const edgeDeg = new Map();
  for (const e of data.edges || []) {
    edgeDeg.set(e.from, (edgeDeg.get(e.from) || 0) + 1);
    edgeDeg.set(e.to, (edgeDeg.get(e.to) || 0) + 1);
  }
  const hideDemo =
    $("graph-hide-demo")?.checked !== false; /* default hide alice/bob seed */
  const bestByLabel = new Map(); // label_lower -> node
  for (const n of data.nodes || []) {
    if (normalizeType(n.type) !== "Person") continue;
    if (n.duplicate_person) continue;
    const lab = String(n.label || n.id || "").toLowerCase();
    if (hideDemo && (lab === "alice" || lab === "bob")) continue;
    const prevN = bestByLabel.get(lab);
    if (!prevN) {
      bestByLabel.set(lab, n);
      continue;
    }
    const dNew = edgeDeg.get(n.id) || 0;
    const dOld = edgeDeg.get(prevN.id) || 0;
    // Prefer more connected; then prefer non-from_team_map; then numeric resource_id
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
  // Rewrite edges that pointed at collapsed people
  const aliasTo = new Map();
  for (const n of data.nodes || []) {
    if (normalizeType(n.type) !== "Person") continue;
    const lab = String(n.label || n.id || "").toLowerCase();
    const keep = bestByLabel.get(lab);
    if (keep && keep.id !== n.id) aliasTo.set(n.id, keep.id);
  }
  if (data.edges) {
    data.edges = data.edges.map((e) => ({
      ...e,
      from: aliasTo.get(e.from) || e.from,
      to: aliasTo.get(e.to) || e.to,
    }));
  }
  const rawNodes = (data.nodes || []).filter((n) => {
    if (normalizeType(n.type) !== "Person") return true;
    return keepPersonIds.has(n.id);
  });
  const nodes = rawNodes.map((n, i) => {
    const type = normalizeType(n.type);
    types.add(type);
    const old = prev.get(n.id);
    const r = nodeRadius(type);
    if (old && !forceLayout) {
      return {
        ...old,
        type,
        label: n.label || n.id,
        r,
        meta: n,
      };
    }
    // seed positions by type rings so multi-person layout is readable
    const angle = (i / Math.max(1, rawNodes.length)) * Math.PI * 2;
    const ring =
      type === "Person" ? 80 :
      type === "Repo" ? 200 :
      type === "Intent" ? 160 :
      120;
    return {
      id: n.id,
      type,
      label: n.label || n.id,
      x: old?.x ?? Math.cos(angle) * ring + (Math.random() - 0.5) * 20,
      y: old?.y ?? Math.sin(angle) * ring + (Math.random() - 0.5) * 20,
      vx: 0,
      vy: 0,
      r,
      fx: null,
      fy: null,
      meta: n,
    };
  });
  graphState.nodes = nodes;
  graphState.edges = (data.edges || []).map((e) => ({
    id: e.id,
    type: e.type || "RELATED",
    from: e.from,
    to: e.to,
    meta: e,
  }));
  // init filters for new types
  for (const t of types) {
    if (graphState.filters[t] === undefined) graphState.filters[t] = true;
  }
  graphState.types = Array.from(types).sort((a, b) => {
    const ia = GRAPH_TYPE_ORDER.indexOf(a);
    const ib = GRAPH_TYPE_ORDER.indexOf(b);
    return (ia < 0 ? 99 : ia) - (ib < 0 ? 99 : ib) || a.localeCompare(b);
  });
  renderGraphFilters();
  if (forceLayout) fitGraph();
}

function renderGraphFilters() {
  const el = $("graph-filters");
  if (!el) return;
  el.innerHTML = graphState.types
    .map((t) => {
      const on = graphState.filters[t] !== false;
      const count = graphState.nodes.filter((n) => n.type === t).length;
      return `<button type="button" class="ghost graph-filter-btn ${on ? "" : "off"}" data-type="${esc(t)}">${esc(t)} (${count})</button>`;
    })
    .join("");
  el.querySelectorAll("[data-type]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const t = btn.getAttribute("data-type");
      graphState.filters[t] = graphState.filters[t] === false;
      renderGraphFilters();
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
          "After a redeploy, only durable journals refill the map. New GitHub activity re-projects within ~2 min. Pre-durability history cannot be restored."
      )} <button type="button" class="ghost" id="btn-banner-seed-intent">Load intent demo</button>`;
      setTimeout(() => {
        const b = $("btn-banner-seed-intent");
        if (b && !b._bound) {
          b._bound = true;
          b.addEventListener("click", () => seedIntentDemo());
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
  const nodes = graphState.nodes.filter((n) => typeVisible(n.type));
  if (nodes.length === 0) return;
  const byId = new Map(graphState.nodes.map((n) => [n.id, n]));
  const edges = graphState.edges.filter(
    (e) => typeVisible(byId.get(e.from)?.type) && typeVisible(byId.get(e.to)?.type)
  );

  // repulsion
  for (let i = 0; i < nodes.length; i++) {
    for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i];
      const b = nodes[j];
      let dx = b.x - a.x;
      let dy = b.y - a.y;
      let dist2 = dx * dx + dy * dy || 0.01;
      const dist = Math.sqrt(dist2);
      const minD = a.r + b.r + 28;
      let force = 900 / dist2;
      if (dist < minD) force += (minD - dist) * 0.15;
      // same-type mild clustering for people
      if (a.type === "Person" && b.type === "Person") force *= 0.55;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;
      a.vx -= fx;
      a.vy -= fy;
      b.vx += fx;
      b.vy += fy;
    }
  }

  // springs
  for (const e of edges) {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    if (!a || !b) continue;
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    const dist = Math.sqrt(dx * dx + dy * dy) || 0.01;
    let ideal = 90;
    if (e.type === "CLAIMS" || e.type === "ABOUT") ideal = 55;
    if (e.type === "BLOCKS" || e.type === "BLOCKED_BY") ideal = 70;
    if (e.type === "AUTHORED" || e.type === "ASSIGNED_TO") ideal = 75;
    if (e.type === "BELONGS_TO") ideal = 100;
    if (e.type === "MEMBER_OF") ideal = 85;
    const k = 0.035;
    const f = (dist - ideal) * k;
    const fx = (dx / dist) * f;
    const fy = (dy / dist) * f;
    a.vx += fx;
    a.vy += fy;
    b.vx -= fx;
    b.vy -= fy;
  }

  // center gravity
  for (const n of nodes) {
    n.vx += -n.x * 0.002;
    n.vy += -n.y * 0.002;
  }

  // integrate
  for (const n of graphState.nodes) {
    if (!typeVisible(n.type)) continue;
    if (n.fx != null) {
      n.x = n.fx;
      n.y = n.fy;
      n.vx = 0;
      n.vy = 0;
      continue;
    }
    n.vx *= 0.82;
    n.vy *= 0.82;
    n.x += n.vx;
    n.y += n.vy;
  }
}

function fitGraph() {
  const nodes = graphState.nodes.filter((n) => typeVisible(n.type));
  const canvas = $("graph-canvas");
  if (!canvas || !nodes.length) {
    graphState.pan = { x: 0, y: 0 };
    graphState.scale = 1;
    return;
  }
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const n of nodes) {
    minX = Math.min(minX, n.x - n.r);
    minY = Math.min(minY, n.y - n.r);
    maxX = Math.max(maxX, n.x + n.r);
    maxY = Math.max(maxY, n.y + n.r);
  }
  const w = maxX - minX || 1;
  const h = maxY - minY || 1;
  const pad = 48;
  const sx = (canvas.width - pad * 2) / w;
  const sy = (canvas.height - pad * 2) / h;
  graphState.scale = Math.min(2.2, Math.max(0.35, Math.min(sx, sy)));
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

  // edges
  for (const e of graphState.edges) {
    const a = byId.get(e.from);
    const b = byId.get(e.to);
    if (!a || !b) continue;
    if (!typeVisible(a.type) || !typeVisible(b.type)) continue;
    ctx.beginPath();
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
    const isBlock = /block/i.test(e.type);
    const isClaim = e.type === "CLAIMS" || e.type === "ABOUT";
    ctx.strokeStyle = isBlock ? "#111" : isClaim ? "#737373" : "#a3a3a3";
    ctx.lineWidth = isBlock ? 1.6 / graphState.scale : 1 / graphState.scale;
    if (isClaim) ctx.setLineDash([4 / graphState.scale, 3 / graphState.scale]);
    else if (isBlock) ctx.setLineDash([2 / graphState.scale, 2 / graphState.scale]);
    else ctx.setLineDash([]);
    ctx.stroke();
    ctx.setLineDash([]);
    // edge label at mid if few edges or selected
    if (
      graphState.edges.length < 40 ||
      a.id === graphState.selected ||
      b.id === graphState.selected
    ) {
      const mx = (a.x + b.x) / 2;
      const my = (a.y + b.y) / 2;
      ctx.font = `${10 / graphState.scale}px ui-sans-serif, system-ui, sans-serif`;
      ctx.fillStyle = "#a3a3a3";
      ctx.textAlign = "center";
      ctx.fillText(e.type, mx, my - 3 / graphState.scale);
    }
  }

  // nodes
  for (const n of graphState.nodes) {
    if (!typeVisible(n.type)) continue;
    const selected = n.id === graphState.selected;
    drawNodeShape(ctx, n, selected);
    // label
    ctx.font = `${11 / graphState.scale}px ui-sans-serif, system-ui, sans-serif`;
    ctx.fillStyle = "#111";
    ctx.textAlign = "center";
    const label = truncateLabel(n.label, 22);
    ctx.fillText(label, n.x, n.y + n.r + 12 / graphState.scale);
    if (n.type === "Intent" && n.meta?.intent_type) {
      ctx.fillStyle = "#737373";
      ctx.font = `${9 / graphState.scale}px ui-monospace, monospace`;
      ctx.fillText(n.meta.intent_type, n.x, n.y + n.r + 22 / graphState.scale);
    }
  }

  ctx.restore();

  // empty state
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
  if (!$("view-graph")?.classList.contains("hidden")) {
    await refreshGraph(false);
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
if ($("btn-gh-app")) {
  $("btn-gh-app").addEventListener("click", () => startOAuth("github"));
}
if ($("btn-team-refresh")) {
  $("btn-team-refresh").addEventListener("click", refreshTeam);
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
if ($("btn-team-add")) {
  $("btn-team-add").addEventListener("click", addTeamMember);
}
if ($("btn-graph-refresh")) {
  $("btn-graph-refresh").addEventListener("click", () => refreshGraph(true));
}
if ($("btn-graph-fit")) {
  $("btn-graph-fit").addEventListener("click", () => {
    fitGraph();
    drawGraph();
  });
}
if ($("graph-hide-demo")) {
  $("graph-hide-demo").addEventListener("change", () => refreshGraph(true));
}
if ($("graph-live")) {
  $("graph-live").addEventListener("change", () => {
    if ($("view-graph")?.classList.contains("hidden")) return;
    stopGraphLive();
    if ($("graph-live").checked) {
      graphState.liveTimer = setInterval(() => refreshGraph(false), 5000);
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
