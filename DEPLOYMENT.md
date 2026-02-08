# Production Deployment Guide

Complete guide for deploying Take It Easy on a VPS.

## Architecture

```
┌─────────────────┐     HTTPS      ┌─────────────────┐
│   Browser       │◄──────────────►│   nginx         │
│   (Elm SPA)     │                │   (reverse      │
└─────────────────┘                │    proxy)       │
                                   └────────┬────────┘
                                            │
                    ┌───────────────────────┼───────────────────────┐
                    │                       │                       │
                    ▼                       ▼                       ▼
            ┌───────────────┐      ┌───────────────┐      ┌───────────────┐
            │ Static Files  │      │ Auth API      │      │ gRPC-Web      │
            │ /             │      │ /auth/*       │      │ /takeiteasygame.*
            │ port 80/443   │      │ port 51051    │      │ port 50052    │
            └───────────────┘      └───────────────┘      └───────────────┘
                                            │
                                            ▼
                                   ┌───────────────┐
                                   │ Rust Backend  │
                                   │ + Graph       │
                                   │   Transformer │
                                   │ + libtorch    │
                                   └───────────────┘
```

## Prerequisites

| Component | Version | Purpose |
|-----------|---------|---------|
| Docker | 20+ | Cross-compilation for glibc 2.35 |
| VPS | Ubuntu 22.04 | Production server (1GB RAM minimum) |
| Domain | Any | FreeDNS offers free subdomains |

## Step 1: Build with Docker

Docker ensures the binary is compatible with Ubuntu 22.04 (glibc 2.35):

```bash
# Copy template and configure
cp build-docker.sh.example build-docker.sh
cp deploy.sh.example deploy.sh

# Edit deploy.sh with your VPS details:
# VPS_USER="root"
# VPS_HOST="your-vps.example.com"
# VPS_PORT="22"

# Build (first run takes ~10 min, subsequent builds ~2 min)
./build-docker.sh
```

This creates:
- `target/release/take_it_easy` - Binary (15 MB)
- `docker-libs/` - libtorch libraries (420 MB)

## Step 2: Deploy to VPS

```bash
# Create deployment package
./deploy.sh package

# Deploy (uploads ~450 MB)
./deploy.sh deploy
```

The deploy script:
1. Uploads binary, libs, frontend, model weights
2. Creates `takeitasy` system user
3. Installs systemd service
4. Configures nginx reverse proxy

## Step 3: Configure HTTPS (Let's Encrypt)

```bash
# SSH to your VPS
ssh user@your-vps.example.com

# Install certbot
apt install certbot python3-certbot-nginx

# Get certificate (auto-configures nginx)
certbot --nginx -d yourdomain.example.com
```

## Step 4: Set JWT Secret (Security)

```bash
# On VPS, generate and set secure JWT secret
JWT_SECRET=$(openssl rand -base64 32)

# Edit the service file
sudo systemctl edit takeitasy --force

# Add:
[Service]
Environment=JWT_SECRET=your-generated-secret-here
Environment=RUST_ENV=production

# Restart
sudo systemctl daemon-reload
sudo systemctl restart takeitasy
```

> **Security**: In production (`RUST_ENV=production`), the server will refuse to start without `JWT_SECRET` set.

## Deployment Commands

| Command | Description |
|---------|-------------|
| `./build-docker.sh` | Build with Docker (glibc 2.35 compat) |
| `./deploy.sh package` | Create deployment package |
| `./deploy.sh deploy` | Full deploy (build + package + upload) |
| `./deploy.sh status` | Check service status |
| `./deploy.sh logs` | View service logs |
| `./deploy.sh restart` | Restart the service |

## File Structure on VPS

```
/opt/takeitasy/
├── take_it_easy          # Rust binary
├── lib/                  # libtorch libraries
│   ├── libtorch_cpu.so
│   ├── libc10.so
│   └── libgomp-*.so
├── model_weights/        # Neural network weights
│   └── graph_transformer_policy.safetensors
├── frontend/             # Elm SPA (static files)
└── data/
    ├── auth.db           # User database (SQLite)
    └── recorded_games/   # Game recordings for AI training
```

## Game Recording

All games are automatically recorded for future AI improvement:

```bash
# Download recorded games from VPS
scp user@vps:/opt/takeitasy/data/recorded_games/*.csv ./recorded_games/

# CSV columns:
# game_id, turn, player_type, plateau_0..18, tile_0..2, position, final_score, human_won
```

## Free Domain with FreeDNS

1. Create account at https://freedns.afraid.org
2. Add subdomain → Type: `AAAA` (for IPv6) or `A` (for IPv4)
3. Point to your VPS IP
4. Update nginx `server_name` directive

## nginx Configuration

Example configuration for HTTPS + gRPC-Web:

```nginx
server {
    listen 443 ssl http2;
    listen [::]:443 ssl http2;
    server_name yourdomain.example.com;

    ssl_certificate /etc/letsencrypt/live/yourdomain.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/yourdomain.example.com/privkey.pem;

    root /opt/takeitasy/frontend;
    index index.html;

    # Frontend SPA
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Auth API
    location /auth/ {
        proxy_pass http://127.0.0.1:51051/auth/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # gRPC-Web
    location /takeiteasygame. {
        proxy_pass http://127.0.0.1:50052;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_buffering off;
        proxy_read_timeout 86400s;
    }
}

server {
    listen 80;
    listen [::]:80;
    server_name yourdomain.example.com;
    return 301 https://$host$request_uri;
}
```

## systemd Service

The service file at `/etc/systemd/system/takeitasy.service`:

```ini
[Unit]
Description=Take It Easy Game Server
After=network.target

[Service]
Type=simple
User=takeitasy
Group=takeitasy
WorkingDirectory=/opt/takeitasy
ExecStart=/opt/takeitasy/take_it_easy --mode multiplayer --port 50051 --nn-architecture graph-transformer
Restart=always
RestartSec=5
Environment=RUST_LOG=info
Environment=RUST_ENV=production
Environment=JWT_SECRET=your-secret-here
Environment=LD_LIBRARY_PATH=/opt/takeitasy/lib

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/takeitasy/data

[Install]
WantedBy=multi-user.target
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| `libtorch_cpu.so not found` | Check `LD_LIBRARY_PATH` in systemd service |
| `GLIBC_2.xx not found` | Rebuild with Docker (ensures glibc 2.35) |
| `JWT_SECRET must be set` | Set `JWT_SECRET` environment variable |
| `502 Bad Gateway` | Check if backend is running: `systemctl status takeitasy` |
| gRPC-Web 405 error | Verify nginx location matches `/takeiteasygame.` |
| CORS errors | Check nginx CORS headers in gRPC-Web location |

## Useful Commands

```bash
# Check service status
systemctl status takeitasy

# View logs
journalctl -u takeitasy -f

# Restart service
systemctl restart takeitasy

# Check nginx config
nginx -t

# Reload nginx
systemctl reload nginx

# Check open ports
ss -tlnp | grep -E ':(80|443|50051|50052|51051)'
```
