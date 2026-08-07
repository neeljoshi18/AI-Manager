#!/usr/bin/env python3
"""
Intent adequacy experiment — data pack exporter (no secrets).

Fetches staging product surfaces for a tenant/subject into a single JSON pack
for offline profile + gap experiments. Best-effort: failed endpoints land in
errors[] without aborting the pack.

Usage:
  python3 scripts/intent_adequacy_pack.py \\
    --base https://status.neel.world \\
    --tenant ten_github \\
    --subject neeljoshi18 \\
    --out plans/packs/2026-08-06_ten_github_neeljoshi18.json

Doctrine:
  - No vault tokens / OAuth secrets
  - OAuth status: booleans + install checklist only
  - No LOC rankings in pack conclusions (meta honesty only)
  - Tag demo/seed intents when detectable
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


META_HONESTY = {
    "multi_repo_poller_fills_graph": True,
    "intent_conflict_may_include_demo_seeds": True,
    "only_mapped_people_get_digests": True,
    "no_private_slack_wiretap": True,
    "no_loc_rankings_allowed_in_conclusions": True,
    "notes": [
        "Multi-repo commit poller + GitHub webhooks fill V2 graph (trajectory/heat).",
        "Intent/conflict v0 is rules-based; seed demo (story-1, intent_demo, graph_story) may still appear.",
        "Only Slack-mapped person twins receive digests / compile content.",
        "No private Slack 1:1 wiretap — channel bot invite is the only future Slack surface.",
        "Do not rank people by LOC, commit counts as virtue, or 'productivity scores'.",
        "Events endpoint may be dominated by ops kinds (sync_graph_to_db) when twin_events is ops-heavy.",
        "PullRequest density may be thin relative to Commit density (poller prioritizes commits).",
    ],
}


def _now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def fetch_json(
    base: str,
    path: str,
    timeout: float = 60.0,
    errors: list[dict[str, Any]] | None = None,
) -> Any | None:
    url = base.rstrip("/") + path
    req = urllib.request.Request(
        url,
        headers={
            "Accept": "application/json",
            "User-Agent": "ai-manager-intent-adequacy-pack/1.0",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            body = resp.read()
            try:
                return json.loads(body.decode("utf-8"))
            except json.JSONDecodeError as e:
                if errors is not None:
                    errors.append(
                        {
                            "path": path,
                            "error": f"json_decode: {e}",
                            "http_status": getattr(resp, "status", None),
                            "bytes": len(body),
                        }
                    )
                return None
    except urllib.error.HTTPError as e:
        detail = e.read()[:500].decode("utf-8", errors="replace") if e.fp else ""
        if errors is not None:
            errors.append(
                {
                    "path": path,
                    "error": f"http_{e.code}",
                    "detail": detail[:300],
                }
            )
        return None
    except Exception as e:  # noqa: BLE001 — best-effort pack
        if errors is not None:
            errors.append({"path": path, "error": type(e).__name__, "detail": str(e)[:300]})
        return None


def oauth_booleans_only(raw: dict[str, Any] | None) -> dict[str, Any] | None:
    """Strip anything that could look like a secret; keep install readiness flags."""
    if not isinstance(raw, dict):
        return raw

    def scrub(obj: Any, key_hint: str = "") -> Any:
        if isinstance(obj, dict):
            out: dict[str, Any] = {}
            for k, v in obj.items():
                kl = k.lower()
                if any(
                    s in kl
                    for s in (
                        "token",
                        "secret",
                        "password",
                        "private_key",
                        "client_secret",
                        "refresh",
                        "authorization",
                    )
                ):
                    # Presence only
                    if isinstance(v, bool):
                        out[k] = v
                    elif v is None:
                        out[k] = False
                    elif isinstance(v, str):
                        out[f"{k}_present"] = bool(v.strip())
                    else:
                        out[f"{k}_present"] = True
                    continue
                out[k] = scrub(v, k)
            return out
        if isinstance(obj, list):
            return [scrub(x) for x in obj]
        if isinstance(obj, str) and len(obj) > 200 and key_hint.lower() not in (
            "note",
            "doctrine",
            "hint",
            "label",
        ):
            return obj[:200] + "…"
        return obj

    keep_keys = (
        "tenant_id",
        "delivery_adapter",
        "delivery_mode",
        "doctrine",
        "public_base_url",
        "slack",
        "github",
        "teams",
        "sso",
        "install_checklist",
        "next_steps",
    )
    slim = {k: raw[k] for k in keep_keys if k in raw}
    return scrub(slim)


def subject_match_tokens(subject: str, member: dict[str, Any] | None) -> set[str]:
    tokens = {subject.lower()}
    if member:
        for k in ("display_name", "subject_id", "slack_user_id", "twin_id"):
            v = member.get(k)
            if isinstance(v, str) and v:
                tokens.add(v.lower())
        for a in member.get("provider_aliases") or []:
            if isinstance(a, str) and a:
                tokens.add(a.lower())
    # common github login form
    tokens.add(subject.lower().replace("_", ""))
    return {t for t in tokens if t}


def find_member(team: dict[str, Any] | None, subject: str) -> dict[str, Any] | None:
    if not team:
        return None
    members = team.get("members") or []
    subj = subject.lower()
    for m in members:
        if (m.get("display_name") or "").lower() == subj:
            return m
        aliases = [str(a).lower() for a in (m.get("provider_aliases") or [])]
        if subj in aliases:
            return m
        if (m.get("subject_id") or "").lower() == subj:
            return m
        if subj in (m.get("twin_id") or "").lower():
            return m
    # fuzzy: subject contained in display_name
    for m in members:
        dn = (m.get("display_name") or "").lower()
        if subj in dn or dn in subj:
            return m
    return None


def person_node_ids_for_subject(
    graph: dict[str, Any] | None,
    subject: str,
    member: dict[str, Any] | None,
) -> set[str]:
    tokens = subject_match_tokens(subject, member)
    ids: set[str] = set()
    if member and member.get("subject_id"):
        sid = member["subject_id"]
        ids.add(f"person:{sid}")
        ids.add(sid)
    nodes = (graph or {}).get("nodes") or []
    for n in nodes:
        if n.get("type") != "Person":
            continue
        blob = " ".join(
            str(n.get(k) or "")
            for k in ("id", "label", "resource_id", "title", "message")
        ).lower()
        if any(t in blob for t in tokens if len(t) >= 3):
            if n.get("id"):
                ids.add(n["id"])
    return ids


def filter_commits_for_subject(
    insights: dict[str, Any] | None,
    graph: dict[str, Any] | None,
    person_ids: set[str],
    subject: str,
) -> dict[str, Any]:
    """Join insights recent_commits with graph AUTHORED edges when possible."""
    recent = list((insights or {}).get("recent_commits") or [])
    nodes = (graph or {}).get("nodes") or []
    edges = (graph or {}).get("edges") or []

    authored_commit_ids: set[str] = set()
    for e in edges:
        if e.get("type") == "AUTHORED" and e.get("from") in person_ids:
            if e.get("to"):
                authored_commit_ids.add(e["to"])

    # Also collect commit nodes whose resource/id mentions subject login (fallback)
    subj_l = subject.lower()
    subject_commit_nodes: list[dict[str, Any]] = []
    for n in nodes:
        if n.get("type") != "Commit":
            continue
        nid = n.get("id") or ""
        rid = n.get("resource_id") or ""
        if nid in authored_commit_ids or rid in authored_commit_ids:
            subject_commit_nodes.append(n)
            continue
        # repo path often owner/repo:sha — owner match is weak signal only
        if nid.startswith(f"commit:{subj_l}/") or f":{subj_l}/" in nid:
            subject_commit_nodes.append(n)

    authored_ids = {n.get("id") for n in subject_commit_nodes if n.get("id")}
    # Match recent_commits by id / resource_id / sha7
    filtered_recent: list[dict[str, Any]] = []
    for c in recent:
        cid = c.get("id")
        rid = c.get("resource_id")
        sha7 = (c.get("sha7") or "")[:7]
        if cid in authored_ids or rid in authored_ids:
            filtered_recent.append(c)
            continue
        if sha7 and any(sha7 in str(n.get("id") or "") or sha7 in str(n.get("label") or "") for n in subject_commit_nodes):
            filtered_recent.append(c)

    # If join found nothing, keep messages that appear on subject-authored nodes
    if not filtered_recent and subject_commit_nodes:
        for n in subject_commit_nodes[:40]:
            msg = n.get("message") or n.get("title") or ""
            filtered_recent.append(
                {
                    "id": n.get("id"),
                    "sha7": n.get("label"),
                    "message": msg,
                    "title": msg,
                    "resource_id": n.get("resource_id"),
                    "filter": "graph_authored",
                }
            )

    return {
        "person_node_ids": sorted(person_ids),
        "authored_commit_node_count": len(authored_ids),
        "recent_commits_unfiltered_count": len(recent),
        "recent_commits_subject": filtered_recent[:40],
        "subject_commit_samples": [
            {
                "id": n.get("id"),
                "label": n.get("label"),
                "message": (n.get("message") or n.get("title") or "")[:200],
                "resource_id": n.get("resource_id"),
            }
            for n in subject_commit_nodes[:40]
        ],
        "filter_note": (
            "recent_commits filtered via Person→AUTHORED→Commit edges when available; "
            "insights/dev recent_commits do not always carry author fields."
        ),
    }


def tag_demoish(obj: Any) -> bool:
    blob = json.dumps(obj, default=str).lower()
    markers = (
        "seed",
        "intent_demo",
        "graph_story",
        "story-1",
        "demo-repo",
        "gu_demo_",
        "demo_alice",
        "demo_bob",
        "rules_demo",
    )
    return any(m in blob for m in markers)


def annotate_intents_conflicts(pulse: dict[str, Any] | None, conflicts: Any) -> dict[str, Any]:
    out: dict[str, Any] = {"pulse_intents": [], "pulse_conflicts": [], "conflicts_proxy": []}
    if isinstance(pulse, dict):
        sample = ((pulse.get("intents") or {}).get("sample")) or []
        for it in sample:
            item = dict(it) if isinstance(it, dict) else {"raw": it}
            item["_demo_seed"] = tag_demoish(it) or (
                isinstance(it, dict)
                and (it.get("properties") or {}).get("seed") is not None
            )
            out["pulse_intents"].append(item)
        cards = ((pulse.get("conflicts") or {}).get("cards")) or []
        for c in cards:
            item = dict(c) if isinstance(c, dict) else {"raw": c}
            item["_demo_seed"] = tag_demoish(c)
            out["pulse_conflicts"].append(item)
    clist = []
    if isinstance(conflicts, dict):
        clist = conflicts.get("conflicts") or []
    elif isinstance(conflicts, list):
        clist = conflicts
    for c in clist:
        item = dict(c) if isinstance(c, dict) else {"raw": c}
        item["_demo_seed"] = tag_demoish(c)
        out["conflicts_proxy"].append(item)
    out["counts"] = {
        "pulse_intents": len(out["pulse_intents"]),
        "pulse_intents_demo_tagged": sum(1 for x in out["pulse_intents"] if x.get("_demo_seed")),
        "pulse_conflicts": len(out["pulse_conflicts"]),
        "pulse_conflicts_demo_tagged": sum(1 for x in out["pulse_conflicts"] if x.get("_demo_seed")),
        "conflicts_proxy": len(out["conflicts_proxy"]),
        "conflicts_proxy_demo_tagged": sum(1 for x in out["conflicts_proxy"] if x.get("_demo_seed")),
    }
    return out


def collect_digests(
    base: str,
    tenant: str,
    team: dict[str, Any] | None,
    subject: str,
    errors: list[dict[str, Any]],
) -> dict[str, Any]:
    members = (team or {}).get("members") or []
    digests: list[dict[str, Any]] = []
    for m in members:
        entry: dict[str, Any] = {
            "display_name": m.get("display_name"),
            "subject_id": m.get("subject_id"),
            "twin_id": m.get("twin_id"),
            "is_subject": (m.get("display_name") or "").lower() == subject.lower()
            or subject.lower() in [str(a).lower() for a in (m.get("provider_aliases") or [])],
            "last_digest_meta": m.get("last_digest"),
            "draft": None,
            "ledger": None,
        }
        ld = m.get("last_digest") or {}
        draft_id = ld.get("draft_id")
        ledger_id = ld.get("ledger_id")
        if draft_id:
            entry["draft"] = fetch_json(
                base, f"/v3/tenants/{tenant}/drafts/{draft_id}", errors=errors
            )
        if ledger_id:
            entry["ledger"] = fetch_json(
                base, f"/v3/tenants/{tenant}/ledgers/{ledger_id}", errors=errors
            )
        digests.append(entry)
    return {
        "mapped_members": len(members),
        "fetched": digests,
        "note": "Digests only exist for mapped person twins; draft+ledger resolved from team.last_digest.",
    }


def graph_neighborhood(
    graph: dict[str, Any] | None,
    person_ids: set[str],
) -> dict[str, Any]:
    if not graph:
        return {"available": False}
    nodes = graph.get("nodes") or []
    edges = graph.get("edges") or []
    node_by_id = {n.get("id"): n for n in nodes if n.get("id")}

    # edges touching person
    touch = [
        e
        for e in edges
        if e.get("from") in person_ids or e.get("to") in person_ids
    ]
    related_ids = set(person_ids)
    for e in touch:
        if e.get("from"):
            related_ids.add(e["from"])
        if e.get("to"):
            related_ids.add(e["to"])

    related_nodes = [node_by_id[i] for i in related_ids if i in node_by_id]
    by_type: dict[str, int] = {}
    for n in related_nodes:
        t = n.get("type") or "?"
        by_type[t] = by_type.get(t, 0) + 1

    repos = sorted(
        {
            (n.get("label") or n.get("resource_id") or n.get("id"))
            for n in related_nodes
            if n.get("type") == "Repo"
        }
    )
    # Repos via commit resource paths
    repo_from_commits: set[str] = set()
    for n in related_nodes:
        if n.get("type") != "Commit":
            continue
        rid = str(n.get("id") or n.get("resource_id") or "")
        # commit:owner/repo:sha
        m = re.match(r"commit:([^:]+:[^:]+):", rid)
        if not m:
            m = re.match(r"commit:([^/]+/[^:]+):", rid)
        if m:
            repo_from_commits.add(m.group(1))
        else:
            # try owner/repo in string
            m2 = re.search(r"([A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+)", rid)
            if m2:
                repo_from_commits.add(m2.group(1))

    edge_by_type: dict[str, int] = {}
    for e in touch:
        t = e.get("type") or "?"
        edge_by_type[t] = edge_by_type.get(t, 0) + 1

    intents = [n for n in related_nodes if n.get("type") == "Intent"]
    prs = [n for n in related_nodes if n.get("type") == "PullRequest"]

    return {
        "available": True,
        "person_node_ids": sorted(person_ids),
        "related_node_count": len(related_nodes),
        "touching_edge_count": len(touch),
        "by_type": by_type,
        "edge_by_type": edge_by_type,
        "repos_direct": repos,
        "repos_from_commits": sorted(repo_from_commits),
        "intents": intents,
        "pull_requests": prs,
        "related_nodes_sample": related_nodes[:80],
        "touching_edges_sample": touch[:120],
        "graph_totals": {
            "nodes_returned": len(nodes),
            "edges_returned": len(edges),
            "by_type": graph.get("by_type"),
            "edge_by_type": graph.get("edge_by_type"),
            "totals": graph.get("totals"),
            "demo_hidden": graph.get("demo_hidden"),
            "include_demo": graph.get("include_demo"),
        },
    }


def summarize_pack(pack: dict[str, Any]) -> dict[str, Any]:
    insights = pack.get("insights_dev") or {}
    graph_info = (insights.get("graph") or {}) if isinstance(insights, dict) else {}
    activity = (insights.get("activity") or {}) if isinstance(insights, dict) else {}
    team = pack.get("team") or {}
    pulse = pack.get("pulse") or {}
    events = pack.get("events") or {}
    digests = pack.get("digests") or {}
    annotated = pack.get("intent_conflict_annotations") or {}
    subject_filter = pack.get("subject_commit_filter") or {}
    return {
        "as_of": pack.get("meta", {}).get("fetched_at"),
        "tenant": pack.get("meta", {}).get("tenant"),
        "subject": pack.get("meta", {}).get("subject"),
        "errors": len(pack.get("errors") or []),
        "graph_nodes_insights": graph_info.get("nodes") or graph_info.get("commit_nodes"),
        "commit_nodes": graph_info.get("commit_nodes"),
        "person_nodes": (graph_info.get("by_type") or {}).get("Person"),
        "repo_nodes": (graph_info.get("by_type") or {}).get("Repo"),
        "intent_nodes": (graph_info.get("by_type") or {}).get("Intent"),
        "pr_nodes": (graph_info.get("by_type") or {}).get("PullRequest"),
        "authored_by": activity.get("by_author"),
        "team_members": len(team.get("members") or []) if isinstance(team, dict) else None,
        "slack_mapped": team.get("slack_mapped_count") if isinstance(team, dict) else None,
        "multi_person_ready": team.get("multi_person_ready") if isinstance(team, dict) else None,
        "pulse_conflict_count": (pulse.get("conflicts") or {}).get("count")
        if isinstance(pulse, dict)
        else None,
        "pulse_intent_count": (pulse.get("intents") or {}).get("count")
        if isinstance(pulse, dict)
        else None,
        "events_count": events.get("count") if isinstance(events, dict) else None,
        "digests_fetched": len(digests.get("fetched") or [])
        if isinstance(digests, dict)
        else None,
        "subject_authored_commits_in_graph_snap": subject_filter.get(
            "authored_commit_node_count"
        ),
        "subject_recent_commits_filtered": len(
            subject_filter.get("recent_commits_subject") or []
        ),
        "demo_tagged": (annotated.get("counts") if isinstance(annotated, dict) else None),
    }


def build_pack(
    base: str,
    tenant: str,
    subject: str,
    graph_node_limit: int = 200,
    graph_edge_limit: int = 500,
) -> dict[str, Any]:
    errors: list[dict[str, Any]] = []
    endpoints: dict[str, str] = {
        "demo_status": "/v3/demo/status",
        "observe_status": "/v3/observe/status",
        "team": f"/v3/tenants/{tenant}/team",
        "pilot_readiness": f"/v3/tenants/{tenant}/pilot_readiness",
        "insights_dev": f"/v3/tenants/{tenant}/insights/dev",
        "pulse": f"/v3/tenants/{tenant}/pulse",
        "graph": (
            f"/v3/tenants/{tenant}/graph"
            f"?node_limit={graph_node_limit}&edge_limit={graph_edge_limit}&include_demo=false"
        ),
        "conflicts": f"/v3/tenants/{tenant}/conflicts",
        "events": f"/v3/tenants/{tenant}/events?limit=50",
        "oauth_status": "/v3/oauth/status",
        "twins": f"/v3/tenants/{tenant}/twins",
    }

    raw: dict[str, Any] = {}
    for key, path in endpoints.items():
        raw[key] = fetch_json(base, path, errors=errors)

    team = raw.get("team") if isinstance(raw.get("team"), dict) else None
    member = find_member(team, subject)
    graph = raw.get("graph") if isinstance(raw.get("graph"), dict) else None
    insights = raw.get("insights_dev") if isinstance(raw.get("insights_dev"), dict) else None
    pulse = raw.get("pulse") if isinstance(raw.get("pulse"), dict) else None
    conflicts = raw.get("conflicts")

    person_ids = person_node_ids_for_subject(graph, subject, member)
    subject_filter = filter_commits_for_subject(insights, graph, person_ids, subject)
    digests = collect_digests(base, tenant, team, subject, errors)
    neighborhood = graph_neighborhood(graph, person_ids)
    annotated = annotate_intents_conflicts(pulse, conflicts)
    oauth = oauth_booleans_only(
        raw.get("oauth_status") if isinstance(raw.get("oauth_status"), dict) else None
    )

    # Subject-scoped insights slice (do not drop full insights; add filtered view)
    insights_subject: dict[str, Any] | None = None
    if insights:
        by_author = (insights.get("activity") or {}).get("by_author") or {}
        insights_subject = {
            "as_of": insights.get("as_of"),
            "subject": subject,
            "subject_authored_count": by_author.get(subject),
            "by_author_all": by_author,
            "hour_of_day_utc": (insights.get("activity") or {}).get("hour_of_day_utc"),
            "day_of_week_utc": (insights.get("activity") or {}).get("day_of_week_utc"),
            "insight": (insights.get("activity") or {}).get("insight"),
            "graph": insights.get("graph"),
            "digests": insights.get("digests"),
            "recent_commits_subject": subject_filter.get("recent_commits_subject"),
            "recent_commits_unfiltered_count": subject_filter.get(
                "recent_commits_unfiltered_count"
            ),
        }

    pack: dict[str, Any] = {
        "meta": {
            "fetched_at": _now_iso(),
            "base": base.rstrip("/"),
            "tenant": tenant,
            "subject": subject,
            "subject_member": {
                "display_name": (member or {}).get("display_name"),
                "subject_id": (member or {}).get("subject_id"),
                "twin_id": (member or {}).get("twin_id"),
                "slack_mapped": (member or {}).get("slack_mapped"),
                "role": (member or {}).get("role"),
                "provider_aliases": (member or {}).get("provider_aliases"),
            }
            if member
            else None,
            "honesty": META_HONESTY,
            "endpoints": endpoints,
            "pack_version": 1,
        },
        "demo_status": raw.get("demo_status"),
        "observe_status": raw.get("observe_status"),
        "team": team,
        "pilot_readiness": raw.get("pilot_readiness"),
        "insights_dev": insights,
        "insights_subject": insights_subject,
        "subject_commit_filter": subject_filter,
        "pulse": pulse,
        "graph_neighborhood": neighborhood,
        # Full graph can be large; keep compact summary + neighborhood samples.
        "graph_summary": {
            "by_type": (graph or {}).get("by_type"),
            "edge_by_type": (graph or {}).get("edge_by_type"),
            "totals": (graph or {}).get("totals"),
            "returned": (graph or {}).get("returned"),
            "nodes_returned": len((graph or {}).get("nodes") or []),
            "edges_returned": len((graph or {}).get("edges") or []),
            "demo_hidden": (graph or {}).get("demo_hidden"),
            "include_demo": (graph or {}).get("include_demo"),
            "team": (graph or {}).get("team"),
            "status": (graph or {}).get("status"),
            "as_of": (graph or {}).get("as_of"),
        }
        if graph
        else None,
        "conflicts": conflicts,
        "intent_conflict_annotations": annotated,
        "events": raw.get("events"),
        "oauth_status": oauth,
        "twins": raw.get("twins"),
        "digests": digests,
        "errors": errors,
    }
    pack["summary"] = summarize_pack(pack)
    return pack


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Export intent-adequacy data pack (no secrets)")
    p.add_argument("--base", default="https://status.neel.world", help="API base URL")
    p.add_argument("--tenant", default="ten_github", help="Tenant id")
    p.add_argument("--subject", default="neeljoshi18", help="Subject login / display name")
    p.add_argument(
        "--out",
        default="",
        help="Output JSON path (default: plans/packs/<date>_<tenant>_<subject>.json)",
    )
    p.add_argument("--graph-node-limit", type=int, default=200)
    p.add_argument("--graph-edge-limit", type=int, default=500)
    args = p.parse_args(argv)

    out = args.out.strip()
    if not out:
        day = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        safe_subj = re.sub(r"[^A-Za-z0-9._-]+", "_", args.subject)
        out = f"plans/packs/{day}_{args.tenant}_{safe_subj}.json"

    out_path = Path(out)
    if out_path.is_dir() or str(out).endswith("/"):
        day = datetime.now(timezone.utc).strftime("%Y-%m-%d")
        safe_subj = re.sub(r"[^A-Za-z0-9._-]+", "_", args.subject)
        out_path = Path(out) / f"{day}_{args.tenant}_{safe_subj}.json"

    pack = build_pack(
        base=args.base,
        tenant=args.tenant,
        subject=args.subject,
        graph_node_limit=args.graph_node_limit,
        graph_edge_limit=args.graph_edge_limit,
    )

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(pack, indent=2, default=str) + "\n", encoding="utf-8")

    summary = pack.get("summary") or {}
    print(f"Wrote: {out_path.resolve()}")
    print("Summary:")
    for k, v in summary.items():
        print(f"  {k}: {v}")
    if pack.get("errors"):
        print(f"Errors ({len(pack['errors'])}):")
        for e in pack["errors"]:
            print(f"  - {e.get('path')}: {e.get('error')} {e.get('detail', '')[:80]}")
    else:
        print("Errors: none")
    return 0


if __name__ == "__main__":
    sys.exit(main())
