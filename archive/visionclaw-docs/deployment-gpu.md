# Docker GPU Deployment Guide

## Prerequisites

### nvidia-container-toolkit

The host must have the NVIDIA Container Toolkit installed. Without it, Docker
cannot expose GPUs to containers.

```bash
# Arch / CachyOS
sudo pacman -S nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker

# Debian / Ubuntu
sudo apt install nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker
```

Verify the toolkit works:

```bash
docker run --rm --gpus all nvidia/cuda:13.0.0-base-ubuntu24.04 nvidia-smi
```

---

## Docker Compose GPU Configuration

`docker-compose.unified.yml` declares GPU access via an extension field:

```yaml
x-gpu-resources: &gpu-resources
  resources:
    reservations:
      devices:
        - driver: nvidia
          count: 1
          capabilities: [gpu, compute, utility]
```

Both the `visionclaw` (dev) and `visionclaw-production` services include:

```yaml
deploy:
  <<: *gpu-resources
runtime: nvidia
```

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `NVIDIA_VISIBLE_DEVICES` | `0` | Which GPU(s) the container sees. Use `0`, `1`, `0,1`, or `all`. |
| `NVIDIA_DRIVER_CAPABILITIES` | `compute,utility` | Exposes compute + nvidia-smi inside the container. |
| `CUDA_ARCH` | `75` | Target SM architecture for NVCC compilation (build arg + env). |

Override in `.env` or inline:

```bash
CUDA_ARCH=86 NVIDIA_VISIBLE_DEVICES=all docker compose --profile dev up --build
```

---

## NVIDIA Driver Compatibility Matrix

| Driver Version | CUDA Toolkit | Max PTX ISA | Status |
|----------------|-------------|-------------|--------|
| 580.x | 13.0 | 9.0 | Minimum supported |
| 595.x | 13.2 | 9.2 | Recommended |

The build system automatically downgrades PTX ISA to 9.0 for maximum driver
compatibility (see `build.rs`, lines 120-130). This means kernels compiled with
CUDA 13.x will JIT-compile on any driver that supports PTX ISA 9.0+.

---

## Build Configuration

### CUDA_ARCH Values

| Value | GPUs | When to Use |
|-------|------|-------------|
| `75` | Turing (RTX 2080, T4) | Default. Portable — PTX JIT-compiles on any sm_75+ GPU. |
| `80` | Ampere (A100) | Known A100 deployment. |
| `86` | Ampere (A6000, RTX 3090) | Known A6000 / RTX 30-series target. |
| `89` | Ada Lovelace (RTX 4090, L40) | Known Ada target. |

### How build.rs Handles Docker vs Host Builds

**Docker builds** (`DOCKER_ENV` is set):
- `nvidia-smi` auto-detection is skipped because the build GPU often differs
  from the runtime GPU (e.g., build on sm_89, deploy on sm_86).
- Falls back to `CUDA_ARCH` env var, defaulting to `75`.

**Host builds** (`DOCKER_ENV` not set):
- Runs `nvidia-smi --query-gpu=compute_cap` to auto-detect the local GPU.
- Falls back to `75` if detection fails.

In both cases, `CUDA_ARCH` env var always takes precedence when set.

### Build Args

```yaml
# docker-compose.unified.yml — development service
build:
  args:
    CUDA_ARCH: ${CUDA_ARCH:-75}
    BUILD_TARGET: development

# production service
build:
  args:
    CUDA_ARCH: ${CUDA_ARCH:-75}
    BUILD_TARGET: production
    REBUILD_PTX: ${REBUILD_PTX:-false}
```

| Build Arg | Default | Purpose |
|-----------|---------|---------|
| `CUDA_ARCH` | `75` | SM architecture passed to nvcc `-arch sm_XX`. |
| `REBUILD_PTX` | `false` | Force PTX recompilation in production builds. |
| `BUILD_TARGET` | `development` | Selects Dockerfile multi-stage target. |

### Production Build with Explicit Architecture

```bash
docker compose --profile prod build \
  --build-arg CUDA_ARCH=86 \
  --build-arg REBUILD_PTX=true
```

---

## Multi-GPU Selection

To restrict which GPUs the container uses:

```bash
# Single GPU (GPU 0)
NVIDIA_VISIBLE_DEVICES=0 docker compose --profile dev up

# Specific GPUs
NVIDIA_VISIBLE_DEVICES=0,2 docker compose --profile dev up

# All GPUs
NVIDIA_VISIBLE_DEVICES=all docker compose --profile dev up
```

Inside the container, verify with:

```bash
nvidia-smi
```

---

## Upgrading the NVIDIA Driver (CachyOS / Arch)

Stop GPU containers first, then upgrade:

```bash
docker stop visionclaw_container agentic-workstation
sudo pacman -Syu nvidia-open-dkms nvidia-utils
sudo reboot
```

After reboot, verify the new driver:

```bash
nvidia-smi
# Check CUDA version shown in top-right of the table
```

Then restart containers:

```bash
docker compose --profile dev up -d
```

### Upgrading on Ubuntu / Debian

```bash
docker stop visionclaw_container agentic-workstation
sudo apt update && sudo apt install --only-upgrade nvidia-driver-595 nvidia-utils-595
sudo reboot
```

---

## Troubleshooting

**Container fails with "no NVIDIA GPU device is present":**
- Verify `nvidia-container-toolkit` is installed.
- Run `nvidia-ctk runtime configure --runtime=docker` and restart Docker.
- Check `docker info | grep -i runtime` shows `nvidia`.

**PTX JIT compilation fails at runtime:**
- Driver is too old for the PTX ISA version. Upgrade the driver (see matrix above).
- Or rebuild with a lower `CUDA_ARCH` value.

**Build fails with "Failed to execute nvcc":**
- CUDA toolkit is not installed in the Docker image. Check the Dockerfile base image.
- Verify `CUDA_PATH` or `CUDA_HOME` points to the toolkit (default: `/opt/cuda`).

**Wrong GPU auto-detected during build:**
- Set `CUDA_ARCH` explicitly. Docker builds already skip auto-detection, but host
  builds will pick up whatever GPU is in slot 0.
