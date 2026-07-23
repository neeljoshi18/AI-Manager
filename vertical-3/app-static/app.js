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
    status: ["My status", "Scheduled digests · veto-first · evidence-backed"],
    connections: ["Connections", "Services and on-demand test status"],
    settings: ["Settings", "Cadence and product boundaries"],
    lab: ["Lab", "Engineer console and raw JSON"],
  };
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
    $("conn-detail").textContent = `Slack: ${h.slack_mode || "—"} · runtime ${h.mode || "—"}`;
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
    if ($("conn-slack")) {
      $("conn-slack").textContent = h.egress
        ? `Egress up · delivery mode: ${h.slack_mode || "—"}. Tokens only in vault.`
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

function renderLatest(payload) {
  if (!payload) return;
  state.latest = payload;
  state.tenant = $("tenant")?.value?.trim() || payload.draft?.tenant_id || state.tenant;
  state.draftId = payload.draft?.draft_id || null;
  state.ledgerId = payload.ledger_id || null;

  const conf = payload.confidence_rollup || payload.ledger?.confidence_rollup || "?";
  const st = payload.draft?.status || "?";
  $("st-conf").textContent = `confidence: ${conf}`;
  $("st-conf").className = "pill " + (conf === "blocker" ? "down" : conf === "high" ? "up" : "mid");
  $("st-status").textContent = `draft: ${st}`;
  $("st-status").className = "pill " + (st === "vetoed" ? "down" : st === "published" ? "up" : "mid");
  $("st-ids").textContent = `ledger=${state.ledgerId || "—"}  draft=${state.draftId || "—"}`;
  $("st-text").textContent = payload.draft?.draft_text || "(no text)";

  const items = payload.ledger?.items || [];
  const blockers = payload.ledger?.open_blockers || [];
  $("st-items").innerHTML = "";
  for (const it of items) {
    const li = document.createElement("li");
    li.innerHTML = `<strong>[${it.confidence}]</strong> ${it.summary}`;
    $("st-items").appendChild(li);
  }
  for (const b of blockers) {
    const li = document.createElement("li");
    li.innerHTML = `<strong>[blocker]</strong> ${b.summary}`;
    $("st-items").appendChild(li);
  }
  if (!items.length && !blockers.length) {
    $("st-items").innerHTML = "<li class='muted'>No items in this window</li>";
  }

  $("today-latest").innerHTML = `
    <div class="meta-row">
      <span class="pill mid">confidence: ${conf}</span>
      <span class="pill mid">draft: ${st}</span>
    </div>
    <pre class="box">${(payload.draft?.draft_text || "").replace(/</g, "&lt;")}</pre>
  `;
  $("lab-raw").textContent = JSON.stringify(payload, null, 2);
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

$("btn-refresh").addEventListener("click", async () => {
  await refreshHealth();
  await refreshOnboarding();
  await loadLatest();
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

refreshHealth();
refreshOnboarding();
loadLatest();
setInterval(refreshHealth, 10000);
setInterval(refreshOnboarding, 15000);
