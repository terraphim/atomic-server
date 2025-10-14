VERSION 0.7
PROJECT atomic-server/firecracker
FROM ubuntu:20.04
ARG TARGETARCH
ARG TARGETOS
ARG TARGETPLATFORM
ARG --global tag=$TARGETOS-$TARGETARCH
ARG --global TARGETARCH
IF [ "$TARGETARCH" = amd64 ]
    ARG --global ARCH=x86_64
ELSE
    ARG --global ARCH=$TARGETARCH
END

# Build all targets for multiple architectures
all:
    BUILD \
        --platform=linux/amd64 \
        --platform=linux/aarch64 \
        +build-firecracker-vm

# Build Atomic Server binary using existing project structure
build-atomic:
  FROM rust:1.81
  RUN rustup target add $ARCH-unknown-linux-musl
  RUN apt update && apt install -y musl-tools musl-dev pkg-config libssl-dev
  RUN update-ca-certificates
  WORKDIR /app
  COPY --dir server lib cli Cargo.toml .
  # Remove old lock file and generate new one for the Rust version
  RUN rm -f Cargo.lock
  RUN cargo fetch
  RUN cargo generate-lockfile --offline
  RUN cargo build --release --bin atomic-server --target $ARCH-unknown-linux-musl
  RUN strip -s /app/target/$ARCH-unknown-linux-musl/release/atomic-server
  SAVE ARTIFACT /app/target/$ARCH-unknown-linux-musl/release/atomic-server AS LOCAL atomic-server-$ARCH
  SAVE ARTIFACT /app/target/$ARCH-unknown-linux-musl/release/atomic-server AS LOCAL ./firecracker/binaries/atomic-server-$ARCH

# Clone Linux kernel source
kernel-source:
  GIT CLONE --branch v5.10 https://github.com/torvalds/linux.git linux.git
  SAVE ARTIFACT linux.git AS LOCAL linux.git

# Build custom kernel for Firecracker
build-kernel:
  FROM +kernel-source
  ENV DEBIAN_FRONTEND noninteractive
  ENV DEBCONF_NONINTERACTIVE_SEEN true
  RUN apt-get update
  RUN DEBIAN_FRONTEND=noninteractive DEBCONF_NONINTERACTIVE_SEEN=true TZ=Etc/UTC apt-get install -yqq --no-install-recommends build-essential bison flex ca-certificates openssl libssl-dev bc wget
  WORKDIR /opt/linux.git
  RUN wget --no-check-certificate https://raw.githubusercontent.com/firecracker-microvm/firecracker/main/resources/guest_configs/microvm-kernel-ci-$ARCH-5.10.config -O .config
  RUN make olddefconfig
  IF [ "$TARGETARCH" = "aarch64" ]
      RUN make -j$(nproc) Image
      SAVE ARTIFACT ./arch/arm64/boot/Image AS LOCAL ./firecracker/kernel/Image-$ARCH
  ELSE
      RUN make -j$(nproc) vmlinux
      SAVE ARTIFACT ./vmlinux AS LOCAL ./firecracker/kernel/vmlinux-$ARCH
  END

# tar2ext4 utility for root filesystem creation
tar2ext4:
    FROM golang:1.21-alpine
    WORKDIR src
    RUN apk add --no-cache git musl-dev
    GIT CLONE https://github.com/microsoft/hcsshim .
    RUN go build ./cmd/tar2ext4
    SAVE ARTIFACT tar2ext4 AS LOCAL ./firecracker/tools/tar2ext4

# Base Atomic Server container for root filesystem
atomic-base:
  FROM ubuntu:20.04
  ENV DEBIAN_FRONTEND noninteractive
  ENV DEBCONF_NONINTERACTIVE_SEEN true
  RUN apt-get update
  RUN DEBIAN_FRONTEND=noninteractive DEBCONF_NONINTERACTIVE_SEEN=true TZ=Etc/UTC apt-get install -yqq --no-install-recommends \
    systemd \
    systemd-sysv \
    udev \
    iproute2 \
    curl \
    dbus \
    kmod \
    iputils-ping \
    net-tools \
    ca-certificates \
    sqlite3 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

  # Configure systemd for container
  RUN rm -f /lib/systemd/system/multi-user.target.wants/systemd-resolved.service
  RUN rm -f /etc/systemd/system/dbus-org.freedesktop.resolve1.service
  RUN rm -f /etc/systemd/system/sysinit.target.wants/systemd-timesyncd.service

  # Create atomic user
  RUN useradd --no-log-init --create-home --shell /bin/bash --home-dir /atomic atomic
  RUN mkdir -p /atomic/data /atomic/config
  RUN chown -R atomic:atomic /atomic

  COPY +build-atomic/atomic-server-$ARCH /usr/bin/atomic-server
  RUN chmod +x /usr/bin/atomic-server

  # Create systemd service using echo
  RUN echo '[Unit]' > /etc/systemd/system/atomic-server.service && \
      echo 'Description=Atomic Server' >> /etc/systemd/system/atomic-server.service && \
      echo 'After=network.target' >> /etc/systemd/system/atomic-server.service && \
      echo '' >> /etc/systemd/system/atomic-server.service && \
      echo '[Service]' >> /etc/systemd/system/atomic-server.service && \
      echo 'Type=simple' >> /etc/systemd/system/atomic-server.service && \
      echo 'User=atomic' >> /etc/systemd/system/atomic-server.service && \
      echo 'Group=atomic' >> /etc/systemd/system/atomic-server.service && \
      echo 'WorkingDirectory=/atomic' >> /etc/systemd/system/atomic-server.service && \
      echo 'ExecStart=/usr/bin/atomic-server --port 8080 --data-dir /atomic/data --config-dir /atomic/config --log-level info' >> /etc/systemd/system/atomic-server.service && \
      echo 'Restart=always' >> /etc/systemd/system/atomic-server.service && \
      echo 'RestartSec=5' >> /etc/systemd/system/atomic-server.service && \
      echo '' >> /etc/systemd/system/atomic-server.service && \
      echo '[Install]' >> /etc/systemd/system/atomic-server.service && \
      echo 'WantedBy=multi-user.target' >> /etc/systemd/system/atomic-server.service

  RUN systemctl enable atomic-server
  RUN systemctl daemon-reload
  RUN systemctl set-default multi-user.target

  SAVE IMAGE atomic-server:base-$ARCH

# Create root filesystem with Atomic Server
create-rootfs:
  FROM +tar2ext4
  WORKDIR /rootfs

  # Create empty filesystem image
  RUN truncate -s 300M rootfs.ext4
  RUN mkfs.ext4 -F rootfs.ext4

  # Mount and populate
  RUN mkdir -p mnt
  WITH DOCKER --load atomic-base:latest=+atomic-base
    RUN mount -o loop rootfs.ext4 mnt
    RUN export CONTAINER_ID=$(docker run -d atomic-base:latest /bin/systemd); \
        docker cp --archive $CONTAINER_ID:/ mnt/ && \
        docker stop $CONTAINER_ID && \
        docker rm $CONTAINER_ID
    RUN umount mnt
  END

  SAVE ARTIFACT rootfs.ext4 AS LOCAL ./firecracker/rootfs/rootfs-$ARCH.ext4

# Assemble Firecracker VM components
build-firecracker-vm:
  FROM alpine:3.18
  WORKDIR /firecracker

  # Copy components
  COPY +build-kernel/* ./kernel/
  COPY +create-rootfs/rootfs.ext4 ./rootfs/
  COPY +build-atomic/atomic-server-$ARCH ./binary/

  # Create VM configuration using echo
  RUN echo '{' > vm-config.json && \
      echo '  "kernel_image_path": "/firecracker/kernel/$(ls kernel/)",' >> vm-config.json && \
      echo '  "boot_args": "console=ttyS0 reboot=k panic=1 pci=off nomodules i8042.nokbd i8042.noaux ipv6.disable=1 systemd.unit=multi-user.target",' >> vm-config.json && \
      echo '  "vcpu_count": 1,' >> vm-config.json && \
      echo '  "mem_size_mib": 256,' >> vm-config.json && \
      echo '  "rootfs_path": "/firecracker/rootfs/rootfs.ext4"' >> vm-config.json && \
      echo '}' >> vm-config.json

  # Save VM artifacts
  SAVE ARTIFACT ./kernel/* AS LOCAL ./firecracker/kernel/
  SAVE ARTIFACT ./rootfs/rootfs.ext4 AS LOCAL ./firecracker/rootfs/
  SAVE ARTIFACT ./vm-config.json AS LOCAL ./firecracker/config/vm-config-$ARCH.json

# Deployment target
deploy:
  FROM +build-firecracker-vm
  RUN echo "Firecracker VM built successfully for $ARCH"
  RUN echo "Kernel: $(ls kernel/)"
  RUN echo "RootFS: $(ls rootfs/)"
  RUN echo "Binary: $(ls binary/)"

# Clean target for local development
clean:
  RUN echo "Cleaning local build artifacts"
  RUN rm -rf ./firecracker/binaries/* ./firecracker/kernel/* ./firecracker/rootfs/*
  RUN echo "Clean completed"