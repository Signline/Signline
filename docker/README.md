# Signline `v0.1.0b` — Optional Docker Deployment

This directory (`docker/`) contains the optional Docker and Docker Compose configuration for running Signline `v0.1.0b` in a container.

> **Note**: For compiling and installing Signline natively on Debian/Ubuntu Linux as a `.deb` package (including the interactive **Ratatui TUI Installer**), see the [`../build_script/`](../build_script/README.md) directory instead.

---

## Contents of `docker/`

```text
docker/
├── Dockerfile           # Multi-stage Dockerfile (Node 20 -> Rust -> Debian Bookworm)
├── docker-compose.yml   # Docker Compose service definition
├── .dockerignore        # Docker build context exclusions
└── README.md            # This guide
```

---

## 1. Run with Docker Compose

From the repository root:

```bash
docker compose -f docker/docker-compose.yml up --build -d
```

Or from inside `docker/`:

```bash
cd docker
docker compose up --build -d
```

---

## 2. Build & Run with `docker` Directly

From the repository root:

```bash
docker build -f docker/Dockerfile -t signline/signline:0.1.0b .
docker run -d -p 3000:3000 -v "$(pwd)/pb_data:/opt/signline/pb_data" --name signline signline/signline:0.1.0b
```
