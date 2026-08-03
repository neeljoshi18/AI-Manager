# Deploy without campus Wi‑Fi SSH (port 22 blocked)

## What’s going wrong

Many campus networks **block outbound TCP 22**. Your laptop can open HTTPS (443) to `status.neel.world`, but `ssh neel@206.189.129.31` **times out**. Mobile hotspot works because it doesn’t block 22.

## Recommended: deploy via GitHub Actions (no SSH from laptop)

You only **`git push` over HTTPS** (allowed on campus). GitHub’s runners SSH to the droplet for you.

### One-time setup (use hotspot once, ~5 minutes)

**Preferred (agent / `gh` CLI — no GitHub UI):**

```bash
# On hotspot: confirm SSH works
ssh -i ~/.ssh/id_ed25519 neel@206.189.129.31 'echo ok'

# Set Actions secrets from your laptop key (once)
gh secret set STAGING_HOST -R neeljoshi18/AI-Manager --body "206.189.129.31"
gh secret set STAGING_USER -R neeljoshi18/AI-Manager --body "neel"
gh secret set STAGING_SSH_KEY -R neeljoshi18/AI-Manager < ~/.ssh/id_ed25519

# Smoke the workflow
gh workflow run deploy-staging.yml -R neeljoshi18/AI-Manager
gh run watch -R neeljoshi18/AI-Manager
```

**Status (2026-08-03):** `STAGING_HOST` / `STAGING_USER` / `STAGING_SSH_KEY` were set via `gh secret set`. Campus path is: **`git push origin main` → Actions deploys** (no laptop SSH).

Manual UI alternative: GitHub → repo → Settings → Secrets → Actions with the same three names.

Prefer a **dedicated deploy key** later (generate `ssh-keygen -t ed25519 -f ~/.ssh/ai_manager_deploy`, put public key in droplet `authorized_keys`, private key only in GitHub secret).

**DigitalOcean firewall**: inbound **TCP 22** allowed from the public internet (GitHub Actions IPs change; allow 22 broadly for this VPS).

After green: from campus, just:
```bash
git push origin main
```
Deploy starts automatically (skips pure markdown path changes).

### Manual trigger (no code change)

```bash
# From Mac with gh CLI logged in (HTTPS):
gh workflow run deploy-staging.yml
```

### What CI does *not* overwrite

`dev_secrets.json` and `deploy/.env.staging` stay **only on the droplet**. CI never has Slack tokens.

---

## Optional: SSH over port 443 (interactive shell on campus)

If you need a shell without hotspot, make `sshd` also listen on **443** (campus almost always allows HTTPS ports).

### One-time on droplet (hotspot)

```bash
ssh -i ~/.ssh/id_ed25519 neel@206.189.129.31
# then:
sudo mkdir -p /etc/ssh/sshd_config.d
echo 'Port 22
Port 443' | sudo tee /etc/ssh/sshd_config.d/99-alt-port.conf
# If Caddy already binds 0.0.0.0:443, you CANNOT use host 443 for sshd.
# Prefer high port instead:
echo 'Port 22
Port 2222' | sudo tee /etc/ssh/sshd_config.d/99-alt-port.conf
sudo systemctl reload ssh || sudo systemctl reload sshd
```

Open **TCP 2222** (or 443 if free) in DigitalOcean firewall.

### On your Mac `~/.ssh/config`

```sshconfig
Host ai-manager-staging
  HostName 206.189.129.31
  User neel
  IdentityFile ~/.ssh/id_ed25519
  Port 2222
  # Fallback when 22 is blocked:
  # Port 2222
```

Then:

```bash
ssh ai-manager-staging
# or force port:
ssh -p 2222 -i ~/.ssh/id_ed25519 neel@206.189.129.31
```

**Note:** Caddy already owns **443** for HTTPS product traffic. Use **2222** (or another free high port), not 443, unless you terminate TLS elsewhere.

---

## Optional: agent path

Coding agents should:

1. Prefer **git push** → Actions deploy (no laptop SSH).
2. Never spam SSH from campus.
3. If interactive SSH is required: ask founder for hotspot **or** use port 2222 if configured.

---

## Decision matrix

| From | Works? | How to deploy |
|------|--------|----------------|
| Campus Wi‑Fi | SSH 22 ❌ | `git push` → GitHub Actions |
| Mobile hotspot | SSH 22 ✅ | `./deploy/scripts/deploy_when_ssh.sh` or Actions |
| Campus + port 2222 open | SSH ✅ if set up | `ssh -p 2222 …` then compose |
