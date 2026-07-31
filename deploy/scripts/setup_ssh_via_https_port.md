# Deploy without campus Wi‑Fi SSH (port 22 blocked)

## What’s going wrong

Many campus networks **block outbound TCP 22**. Your laptop can open HTTPS (443) to `status.neel.world`, but `ssh neel@206.189.129.31` **times out**. Mobile hotspot works because it doesn’t block 22.

## Recommended: deploy via GitHub Actions (no SSH from laptop)

You only **`git push` over HTTPS** (allowed on campus). GitHub’s runners SSH to the droplet for you.

### One-time setup (use hotspot once, ~10 minutes)

1. **Confirm the droplet accepts your key** (on hotspot):
   ```bash
   ssh -i ~/.ssh/id_ed25519 neel@206.189.129.31 'echo ok'
   ```

2. **GitHub → repo `neeljoshi18/AI-Manager` → Settings → Secrets and variables → Actions → New repository secret**

   | Name | Value |
   |------|--------|
   | `STAGING_HOST` | `206.189.129.31` |
   | `STAGING_USER` | `neel` |
   | `STAGING_SSH_KEY` | Entire contents of `~/.ssh/id_ed25519` (private key, including `BEGIN`/`END` lines) |

   Prefer a **dedicated deploy key** later (generate `ssh-keygen -t ed25519 -f ~/.ssh/ai_manager_deploy`, put public key in droplet `authorized_keys`, private key only in GitHub secret).

3. **DigitalOcean firewall**: inbound **TCP 22** allowed from the public internet (or at least not locked to campus only). GitHub Actions IPs change often; allow 22 broadly for this VPS.

4. **Test**: GitHub → **Actions → Deploy staging → Run workflow**

5. After green: from campus, just push code:
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
