const $ = (id) => document.getElementById(id);

let state = {
  tenant: "ten_demo",
  draftId: null,
  ledgerId: null,
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
  if (!res.ok) {
    throw new Error(body.error || body.raw || res.statusText);
  }
  return body;
}

function pill(name, up) {
  const cls = up === true ? "up" : up === false ? "down" : "mid";
  const label = up === true ? "up" : up === false ? "down" : "n/a";
  return `<span class="pill ${cls}">${name}: ${label}</span>`;
}

async function refreshHealth() {
  try {
    const h = await jfetch("/v3/demo/status");
    $("health-pills").innerHTML = [
      pill("V3", true),
      pill("V1", h.v1),
      pill("V2", h.v2),
      pill("egress", h.egress),
    ].join("");
    const pathHint =
      h.v1 && h.v2
        ? " · full stack: V1 ingest + V2 graph available"
        : h.v2
          ? " · V2 graph available (V1 still down)"
          : " · demo can use fixtures if V2 down";
    $("slack-mode").textContent = h.slack_mode
      ? `Slack delivery: ${h.slack_mode}${h.mode ? ` · runtime ${h.mode}` : ""}${pathHint}`
      : pathHint;
  } catch (e) {
    $("health-pills").innerHTML = pill("V3", false);
    $("slack-mode").textContent = String(e.message || e);
  }
}

function renderLedger(payload) {
  state.tenant = $("tenant").value.trim() || "ten_demo";
  state.draftId = payload.draft?.draft_id || null;
  state.ledgerId = payload.ledger_id || payload.ledger?.ledger_id || null;

  $("ledger-empty").classList.add("hidden");
  $("ledger-view").classList.remove("hidden");

  const conf = payload.confidence_rollup || payload.ledger?.confidence_rollup || "?";
  const st = payload.draft?.status || "?";
  $("conf-pill").textContent = `confidence: ${conf}`;
  $("conf-pill").className = "pill " + (conf === "blocker" ? "down" : conf === "high" ? "up" : "mid");
  $("draft-pill").textContent = `draft: ${st}`;
  $("draft-pill").className = "pill " + (st === "vetoed" ? "down" : st === "published" ? "up" : "mid");
  $("ids").textContent = `ledger=${state.ledgerId || "—"}  draft=${state.draftId || "—"}`;

  $("draft-text").textContent = payload.draft?.draft_text || payload.draft_text || "(no text)";

  const items = payload.ledger?.items || payload.ledger?.ledger?.items || [];
  const openBlockers = payload.ledger?.open_blockers || payload.ledger?.ledger?.open_blockers || [];
  $("items").innerHTML = "";
  for (const it of items) {
    const li = document.createElement("li");
    li.innerHTML = `<strong>[${it.confidence}]</strong> ${it.summary} <code>${it.node_id}</code>`;
    $("items").appendChild(li);
  }
  for (const b of openBlockers) {
    const li = document.createElement("li");
    li.innerHTML = `<strong>[blocker]</strong> ${b.summary} <code>${b.node_id}</code>`;
    $("items").appendChild(li);
  }
  if (!items.length && !openBlockers.length) {
    $("items").innerHTML = "<li class='muted'>No code signals (honest empty ledger)</li>";
  }

  const refs = [];
  for (const it of items) {
    for (const r of it.evidence_refs || []) refs.push(r);
  }
  $("evidence").textContent = refs.length ? refs.join("\n") : "(none)";
  $("raw").textContent = JSON.stringify(payload, null, 2);
}

async function simulate() {
  $("btn-sim").disabled = true;
  $("btn-sim").textContent = "Running…";
  try {
    const body = {
      tenant_id: $("tenant").value.trim() || "ten_demo",
      global_user_id: $("user").value.trim() || "gu_alice",
      display_name: $("name").value.trim() || "Alice",
      slack_user_id: $("slack_user").value.trim() || "U_DEMO",
      channel_id: $("channel").value.trim() || "C_DEMO",
      skip_shadow: $("skip_shadow").checked,
      pr_title: "Demo: fix auth race",
      resource_id: "acme/app/pr/7",
    };
    const payload = await jfetch("/v3/demo/simulate", {
      method: "POST",
      body: JSON.stringify(body),
    });
    renderLedger(payload);
  } catch (e) {
    alert("Simulate failed: " + (e.message || e));
  } finally {
    $("btn-sim").disabled = false;
    $("btn-sim").textContent = "Simulate PR → Ledger → Draft";
  }
}

async function act(kind) {
  if (!state.draftId) {
    alert("No draft yet — run Simulate first");
    return;
  }
  const base = `/v3/tenants/${encodeURIComponent(state.tenant)}/drafts/${encodeURIComponent(state.draftId)}`;
  try {
    let payload;
    if (kind === "edit") {
      const text = prompt("Edited status text:", $("draft-text").textContent);
      if (text == null) return;
      payload = await jfetch(base + "/edit", { method: "POST", body: JSON.stringify({ text }) });
      // after edit, show draft; user may publish
      const full = await jfetch(`/v3/demo/latest?tenant_id=${encodeURIComponent(state.tenant)}`);
      if (full && full.draft) renderLedger(full);
      else renderLedger({ draft: payload, ledger_id: state.ledgerId, confidence_rollup: "?" });
      return;
    }
    payload = await jfetch(base + "/" + kind, { method: "POST", body: kind === "edit" ? undefined : "{}" });
    // silence/publish return {draft, publish}
    const draft = payload.draft || payload;
    const latest = await jfetch(`/v3/demo/latest?tenant_id=${encodeURIComponent(state.tenant)}`).catch(() => null);
    if (latest && latest.draft) renderLedger(latest);
    else
      renderLedger({
        draft,
        ledger_id: state.ledgerId,
        confidence_rollup: latest?.confidence_rollup,
        publish: payload.publish,
      });
  } catch (e) {
    alert(kind + " failed: " + (e.message || e));
  }
}

$("btn-sim").addEventListener("click", simulate);
$("btn-publish").addEventListener("click", () => act("publish"));
$("btn-veto").addEventListener("click", () => act("veto"));
$("btn-silence").addEventListener("click", () => act("silence"));
$("btn-edit").addEventListener("click", () => act("edit"));

refreshHealth();
setInterval(refreshHealth, 8000);
