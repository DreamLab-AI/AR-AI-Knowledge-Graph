# Stage 0 – CUDA 12.9 + cuDNN (official NVIDIA image)
FROM nvidia/cuda:12.9.0-cudnn-devel-ubuntu24.04 AS base

################################################################################
# Stage 1 – OS deps, Python 3.12 & 3.13 venvs, Rust, Node, ML stack, WasmEdge (Blender disabled)
################################################################################
ARG DEBIAN_FRONTEND=noninteractive

# 1. Set Environment Variables for Blender (COMMENTED OUT - Blender disabled)
# Set the Blender version and create a directory for it.
# ARG BLENDER_DOWNLOAD_URL
# ENV BLENDER_VERSION="4.5"
# ENV BLENDER_PATH="/usr/local/blender"

# Set the application workspace directory
ENV APP_HOME="/app"
WORKDIR $APP_HOME

# 2. Install Dependencies
# Add any dependencies needed to run Blender and its Python environment.
# wget and other utilities are for downloading and extracting Blender.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates curl gnupg software-properties-common
# Add Docker's official GPG key
RUN install -m 0755 -d /etc/apt/keyrings && \
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg && \
    chmod a+r /etc/apt/keyrings/docker.gpg
# Set up the repository
RUN echo \
      "deb [arch="$(dpkg --print-architecture)" signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
      "$(. /etc/os-release && echo "$VERSION_CODENAME")" stable" | \
      tee /etc/apt/sources.list.d/docker.list > /dev/null
# Add Deadsnakes PPA for newer Python versions
RUN add-apt-repository -y ppa:deadsnakes/ppa
# Add NodeSource repository for up-to-date NodeJS (v22+)
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash -
# Install all packages including network utilities
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      wget libxi6 libxxf86vm1 libxfixes3 libxrender1 \
      build-essential clang git pkg-config libssl-dev \
      lsb-release shellcheck hyperfine openssh-client tmux sudo \
      docker-ce docker-ce-cli containerd.io unzip 7zip texlive-full latexmk chktex \
      # Network utilities for debugging
      iputils-ping netcat-openbsd net-tools dnsutils traceroute tcpdump nmap \
      iproute2 iptables curl wget telnet mtr-tiny \
      # Additional Blender dependencies for headless operation (COMMENTED OUT)
      # libgl1 libglu1-mesa libglib2.0-0 libsm6 libxext6 \
      # libfontconfig1 libxkbcommon0 libxkbcommon-x11-0 libdbus-1-3 \
      # X11 virtual framebuffer for headless rendering
      xvfb \
      # Python, Node, and GPU/Wasm dependencies
      python3.12 python3.12-venv python3.12-dev \
      python3.13 python3.13-venv python3.13-dev \
      nodejs \
      jq \
      libvulkan1 vulkan-tools ocl-icd-libopencl1 && \
    rm -rf /var/lib/apt/lists/* && \
    # Linters
    wget -O /usr/local/bin/hadolint https://github.com/hadolint/hadolint/releases/download/v2.12.0/hadolint-Linux-x86_64 && \
    chmod +x /usr/local/bin/hadolint

# 3. Install Blender
# Download and extract the specified Blender LTS version.
# Use default URL if not provided
# RUN BLENDER_URL=${BLENDER_DOWNLOAD_URL:-"https://mirror.clarkson.edu/blender/release/Blender4.5/blender-4.5.0-linux-x64.tar.xz"} && \
#     wget "${BLENDER_URL}" -O blender.tar.xz && \
#     tar -xf blender.tar.xz && \
#     mv blender-${BLENDER_VERSION}.0-linux-x64 ${BLENDER_PATH} && \
#     rm blender.tar.xz

# 4. Create addon directory and copy files later (COMMENTED OUT - Blender disabled)
# We'll copy the addon files after creating the proper directory structure
# RUN mkdir -p ${BLENDER_PATH}/${BLENDER_VERSION}/scripts/addons/addon

# 5. Install the MCP Server Package Dependencies (COMMENTED OUT - Blender disabled)
# Install dependencies for the addon using Blender's Python
# RUN /usr/local/blender/4.5/python/bin/python3.11 -m ensurepip && \
#     /usr/local/blender/4.5/python/bin/python3.11 -m pip install --upgrade pip && \
#     /usr/local/blender/4.5/python/bin/python3.11 -m pip install Pillow

# 6. Set PYTHONPATH for Blender integration
ENV PYTHONPATH="${APP_HOME}"

# 7. Copy startup and addon files (MCP server files for remote Blender connection)
# Note: addon.py is used as MCP server to connect to remote Blender, not as local addon
COPY addon.py $APP_HOME/
COPY keep_alive.py $APP_HOME/
COPY entrypoint.sh /
COPY entrypoint.sh /
RUN chmod +x /entrypoint.sh

# ---------- Create Python virtual environments & install global node packages ----------
RUN python3.12 -m venv /opt/venv312 && \
    /opt/venv312/bin/pip install --upgrade pip wheel setuptools && \
    python3.13 -m venv /opt/venv313 && \
    /opt/venv313/bin/pip install --upgrade pip wheel setuptools

# ---------- Install global CLI tools with specific versions ----------
RUN npm install -g \
    claude-flow@alpha \
    ruv-swarm@latest \
    @anthropic-ai/claude-code@latest \
    @google/gemini-cli@latest \
    @openai/codex@latest

# ---------- Install Python ML & AI libraries into the 3.12 venv ----------
# Copy requirements file and install dependencies to leverage Docker layer caching.
COPY requirements.txt .
RUN /opt/venv312/bin/pip install --no-cache-dir --retries 10 --timeout 60 -r requirements.txt

# Install the Modular MAX runtime last. As a pre-release, it's best
# installed on its own to avoid influencing the resolution of stable packages.
RUN /opt/venv312/bin/pip install --no-cache-dir --retries 10 --timeout 60 --pre modular

# ---------- Rust tool-chain (AVX‑512) ----------
ENV PATH="/root/.cargo/bin:${PATH}"
# Update certificates and install Rust with retry logic
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates && \
    update-ca-certificates && \
    for i in 1 2 3; do \
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile default && break || \
        echo "Rust installation attempt $i failed, retrying..." && sleep 5; \
    done && \
    echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> /etc/profile.d/rust.sh && \
    . "$HOME/.cargo/env" && \
    cargo install cargo-edit
ENV RUSTFLAGS="-C target-cpu=skylake-avx512 -C target-feature=+avx2,+avx512f,+avx512bw,+avx512dq"

# ---------- Install uv (fast python package manager) and uvx ----------
RUN curl -LsSf https://astral.sh/uv/install.sh | sh && \
    echo 'export PATH="/root/.local/bin:$PATH"' >> /etc/profile.d/uv.sh

# ---------- GPU‑accelerated Wasm stack (WasmEdge) ----------
RUN curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | \
    bash -s -- -p /usr/local --plugins wasi_nn-openvino && ldconfig

# ---------- OpenVINO from official APT repository ----------
RUN wget -qO- https://apt.repos.intel.com/intel-gpg-keys/GPG-PUB-KEY-INTEL-SW-PRODUCTS.PUB | \
    gpg --dearmor --output /etc/apt/trusted.gpg.d/intel.gpg && \
    echo "deb https://apt.repos.intel.com/openvino ubuntu24 main" > /etc/apt/sources.list.d/intel-openvino.list && \
    apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends openvino-2025.2.0

################################################################################
# Stage 2 – Non‑root user, health‑check, env placeholders
################################################################################
ARG UID=1000
ARG GID=1000
# Remove the existing ubuntu user and replace it with the dev user
# This ensures there's no UID conflict and the dev user is properly used
RUN (id ubuntu &>/dev/null && userdel -r ubuntu) || true && \
    groupadd -g ${GID} dev && \
    useradd -m -s /bin/bash -u ${UID} -g ${GID} dev && \
    # Add dev user to the docker and sudo groups
    usermod -aG docker,sudo dev && \
    # Allow passwordless sudo for the dev user
    echo "dev ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/dev && chmod 0440 /etc/sudoers.d/dev && \
    # Fix ownership of npm global modules so dev user can write to them
    chown -R dev:dev /usr/lib/node_modules && \
    # Create python symlink for convenience
    ln -s /usr/bin/python3.12 /usr/local/bin/python && \
    # Create workspace directories with proper ownership
    mkdir -p /workspace /workspace/ext /workspace/logs /workspace/.claude /workspace/.mcp /workspace/memory && \
    chown -R dev:dev /workspace && \
    # Make uv accessible to dev user
    cp -r /root/.local /home/dev/ && \
    chown -R dev:dev /home/dev/.local && \
    echo 'export PATH="/home/dev/.local/bin:$PATH"' >> /home/dev/.bashrc


USER dev
WORKDIR /workspace
COPY README.md .
COPY CLAUDE-README.md .

# Configure git for the dev user
RUN git config --global user.email "swarm@dreamlab-ai.com" && \
    git config --global user.name "Swarm Agent"

# Activate 3.12 venv by default
ENV PATH="/opt/venv312/bin:/home/dev/.local/bin:${PATH}"

# Runtime placeholders
ENV WASMEDGE_PLUGIN_PATH="/usr/local/lib/wasmedge"

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
  CMD ["sh", "-c", "command -v claude >/dev/null && command -v claude-flow >/dev/null"] || exit 1

# Start via entrypoint.sh which handles all services including Blender MCP
ENTRYPOINT ["/entrypoint.sh"]
CMD ["--interactive"]