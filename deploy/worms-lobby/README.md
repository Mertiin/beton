# Worms lobby on Coolify (same Hetzner as Hotpot)

Hotpot is a Coolify **Dockerfile** app on [ci.itknegaren.com](https://ci.itknegaren.com) → `hotpot.itknegaren.com`.
Worms lobby should be the same: **Coolify builds the image from git** — not a hand-rolled `/opt` compose.

## DNS

| Name | Type | Value |
|------|------|--------|
| `worms` | A | `37.27.220.29` |

→ `worms.itknegaren.com`

## Create the Coolify resource

1. Open [Coolify](https://ci.itknegaren.com) → project (same server as Hotpot).
2. **+ New** → **Docker Compose** (needed so TCP/UDP `7800–7899` can be published for WebRTC).
3. Connect the **brutal-gfx / beton** git repo (branch `main`).
4. Compose file: `docker-compose.worms-lobby.yml` (repo root).
5. Domain: `worms.itknegaren.com` → service `worms-lobby` port **8080**.
6. Environment:

```text
PUBLIC_WS_BASE=ws://37.27.220.29
BETON_PUBLIC_IP=37.27.220.29
```

7. Deploy. Coolify builds `deploy/worms-lobby/Dockerfile` and routes HTTPS like Hotpot.

### Alternative: single Dockerfile app

If you prefer **Dockerfile** (like Hotpot) instead of Compose:

- Base directory: `/`
- Dockerfile: `deploy/worms-lobby/Dockerfile`
- Port: `8080`
- Add **port mappings** `7800-7899` TCP **and** UDP in Coolify
- Same env vars as above

## GitHub deploy hook (optional, Hotpot-style)

After the Coolify app exists, copy its **Deploy Webhook** + API token into GitHub secrets
`COOLIFY_WORMS_WEBHOOK` / `COOLIFY_TOKEN` (or reuse `COOLIFY_WEBHOOK` / `COOLIFY_TOKEN`).
Workflow: `.github/workflows/deploy-worms-lobby.yml`.

## Clients

```powershell
$env:WORMS_LOBBY="https://worms.itknegaren.com"
cargo run -p worms --bin worms
```
