#!/bin/bash

# Atomic Server Firecracker VM Deployment Script
# This script starts an Atomic Server instance in a Firecracker microVM

set -euo pipefail

# Configuration
VM_ID="${1:-atomic-server-$(date +%s)}"
KERNEL_PATH="./firecracker/kernel/vmlinux-x86_64"
ROOTFS_PATH="./firecracker/rootfs/rootfs-x86_64.ext4"
BINARY_PATH="./firecracker/binaries/atomic-server-x86_64"
FC_BINARY="firecracker"
# Use shorter TAP device name (max 15 chars)
TAP_SUFFIX=$(echo "$VM_ID" | md5sum | cut -c1-8)
TAP_DEV="fc-${TAP_SUFFIX}"
VM_IP="169.254.100.2"
TAP_IP="169.254.100.1"
VM_MAC="02:FC:00:00:00:01"
FC_PORT="8080"
API_SOCKET="/tmp/firecracker-${VM_ID}.sock"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1${NC}"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1${NC}"
    exit 1
}

# Check prerequisites
check_prerequisites() {
    log "Checking prerequisites..."

    # Check if Firecracker is installed
    if ! command -v $FC_BINARY &> /dev/null; then
        error "Firecracker binary '$FC_BINARY' not found. Please install Firecracker."
    fi

    # Check if required files exist
    [[ -f "$KERNEL_PATH" ]] || error "Kernel not found at $KERNEL_PATH"
    [[ -f "$ROOTFS_PATH" ]] || error "Root filesystem not found at $ROOTFS_PATH"
    [[ -f "$BINARY_PATH" ]] || error "Atomic Server binary not found at $BINARY_PATH"

    # Check if running as root (required for TAP devices)
    [[ $EUID -eq 0 ]] || warn "Not running as root. TAP device creation may fail."

    log "Prerequisites check passed."
}

# Setup network infrastructure
setup_network() {
    log "Setting up network infrastructure..."

    # Clean up existing TAP device if it exists
    if ip link show "$TAP_DEV" &>/dev/null; then
        log "Removing existing TAP device $TAP_DEV"
        ip link del "$TAP_DEV"
    fi

    # Create TAP device
    ip tuntap add dev "$TAP_DEV" mode tap || error "Failed to create TAP device $TAP_DEV"

    # Configure TAP device
    ip addr add "${TAP_IP}/30" dev "$TAP_DEV"
    ip link set dev "$TAP_DEV" up
    sysctl -w net.ipv4.conf.${TAP_DEV}.proxy_arp=1 >/dev/null
    sysctl -w net.ipv6.conf.${TAP_DEV}.disable_ipv6=1 >/dev/null

    log "Network setup complete: $TAP_DEV ($TAP_IP) -> VM ($VM_IP)"
}

# Prepare custom root filesystem with Atomic Server
prepare_rootfs() {
    log "Preparing root filesystem with Atomic Server..."

    # Create temporary mount point
    TEMP_MNT=$(mktemp -d -p /var/tmp)
    trap "umount $TEMP_MNT 2>/dev/null || true; rmdir $TEMP_MNT 2>/dev/null || true" EXIT

    # Mount root filesystem
    mount -o loop "$ROOTFS_PATH" "$TEMP_MNT" || error "Failed to mount root filesystem"

    # Copy Atomic Server binary
    cp "$BINARY_PATH" "$TEMP_MNT/usr/bin/atomic-server"
    chmod +x "$TEMP_MNT/usr/bin/atomic-server"

    # Create Atomic Server systemd service
    cat > "$TEMP_MNT/etc/systemd/system/atomic-server.service" << EOF
[Unit]
Description=Atomic Server in Firecracker VM
After=network.target

[Service]
Type=simple
User=atomic
Group=atomic
WorkingDirectory=/atomic
ExecStart=/usr/bin/atomic-server --port 8080 --data-dir /atomic/data --config-dir /atomic/config --log-level info
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

    # Enable the service
    chroot "$TEMP_MNT" systemctl enable atomic-server >/dev/null 2>&1 || true

    # Create atomic user and directories if they don't exist
    chroot "$TEMP_MNT" useradd --no-log-init --create-home --shell /bin/bash atomic 2>/dev/null || true
    mkdir -p "$TEMP_MNT/atomic/data" "$TEMP_MNT/atomic/config"
    chown -R atomic:atomic "$TEMP_MNT/atomic"

    # Unmount
    umount "$TEMP_MNT"
    rmdir "$TEMP_MNT"

    log "Root filesystem preparation complete."
}

# Start Firecracker VM
start_firecracker() {
    log "Starting Firecracker VM..."

    # Remove existing API socket
    rm -f "$API_SOCKET"

    # Start Firecracker in background
    $FC_BINARY --api-sock "$API_SOCKET" --id "$VM_ID" > "/tmp/firecracker-${VM_ID}.log" 2>&1 &
    FC_PID=$!

    # Wait for API server to start
    local retries=50
    while [[ $retries -gt 0 ]] && [[ ! -e "$API_SOCKET" ]]; do
        sleep 0.1
        ((retries--))
    done

    if [[ ! -e "$API_SOCKET" ]]; then
        error "Firecracker API server failed to start"
    fi

    log "Firecracker started (PID: $FC_PID)"
}

# Configure VM via API
configure_vm() {
    log "Configuring VM via Firecracker API..."

    # Helper function for API calls
    curl_put() {
        local url="$1"
        local data="$2"
        local response
        response=$(curl -s --show-error --header "Content-Type: application/json" \
                    --unix-socket "$API_SOCKET" -X PUT --data "$data" \
                    "http://localhost/${url#/}" 2>&1)

        if [[ $? -ne 0 ]]; then
            error "API call failed: $response"
        fi

        echo "$response"
    }

    # Set up logger
    curl_put "/logger" '{
        "level": "Info",
        "log_path": "/tmp/firecracker-'$VM_ID'-vm.log",
        "show_level": false,
        "show_log_origin": false
    }' > /dev/null

    # Configure machine
    curl_put "/machine-config" '{
        "vcpu_count": 1,
        "mem_size_mib": 256,
        "track_dirty_pages": false
    }' > /dev/null

    # Set boot source
    curl_put "/boot-source" '{
        "kernel_image_path": "'$KERNEL_PATH'",
        "boot_args": "console=ttyS0 reboot=k panic=1 pci=off nomodules i8042.nokbd i8042.noaux ipv6.disable=1 systemd.unit=multi-user.target ip='$VM_IP'::'$TAP_IP'/30::eth0:off"
    }' > /dev/null

    # Attach root filesystem
    curl_put "/drives/rootfs" '{
        "drive_id": "rootfs",
        "path_on_host": "'$ROOTFS_PATH'",
        "is_root_device": true,
        "is_read_only": false
    }' > /dev/null

    # Configure network interface
    curl_put "/network-interfaces/eth0" '{
        "iface_id": "eth0",
        "guest_mac": "'$VM_MAC'",
        "host_dev_name": "'$TAP_DEV'"
    }' > /dev/null

    log "VM configuration complete."
}

# Start the VM
start_vm() {
    log "Starting VM instance..."

    curl -s --show-error --header "Content-Type: application/json" \
         --unix-socket "$API_SOCKET" -X PUT --data '{"action_type": "InstanceStart"}' \
         "http://localhost/actions" > /dev/null

    if [[ $? -ne 0 ]]; then
        error "Failed to start VM instance"
    fi

    log "VM instance starting..."
}

# Wait for Atomic Server to be ready
wait_for_atomic() {
    log "Waiting for Atomic Server to be ready..."

    local retries=60
    while [[ $retries -gt 0 ]]; do
        if curl -s --connect-timeout 2 "http://$VM_IP:$FC_PORT" >/dev/null 2>&1; then
            log "Atomic Server is ready at http://$VM_IP:$FC_PORT"
            return 0
        fi

        sleep 2
        ((retries--))
        echo -n "."
    done

    echo
    error "Atomic Server failed to start within timeout"
}

# Setup host port forwarding
setup_port_forwarding() {
    log "Setting up port forwarding..."

    # Allow IP forwarding
    sysctl -w net.ipv4.ip_forward=1 >/dev/null

    # Set up NAT and port forwarding
    iptables -t nat -A PREROUTING -p tcp --dport 8080 -j DNAT --to-destination $VM_IP:8080
    iptables -t nat -A POSTROUTING -s $VM_IP -j MASQUERADE
    iptables -A FORWARD -d $VM_IP -p tcp --dport 8080 -j ACCEPT

    log "Port forwarding configured: localhost:8080 -> VM:$VM_IP:8080"
}

# Cleanup function
cleanup() {
    log "Cleaning up..."

    # Kill Firecracker process
    if [[ -n "${FC_PID:-}" ]]; then
        kill $FC_PID 2>/dev/null || true
        wait $FC_PID 2>/dev/null || true
    fi

    # Clean up network
    if ip link show "$TAP_DEV" &>/dev/null; then
        ip link del "$TAP_DEV"
    fi

    # Clean up iptables rules
    iptables -t nat -D PREROUTING -p tcp --dport 8080 -j DNAT --to-destination $VM_IP:8080 2>/dev/null || true
    iptables -t nat -D POSTROUTING -s $VM_IP -j MASQUERADE 2>/dev/null || true
    iptables -D FORWARD -d $VM_IP -p tcp --dport 8080 -j ACCEPT 2>/dev/null || true

    # Remove API socket
    rm -f "$API_SOCKET"

    log "Cleanup complete."
}

# Set up signal handlers
trap cleanup EXIT INT TERM

# Main execution
main() {
    log "Starting Atomic Server Firecracker VM deployment..."
    log "VM ID: $VM_ID"

    check_prerequisites
    setup_network
    prepare_rootfs
    start_firecracker
    configure_vm
    start_vm
    wait_for_atomic
    setup_port_forwarding

    log ""
    log "🎉 Atomic Server is now running in Firecracker VM!"
    log "📊 VM ID: $VM_ID"
    log "🌐 Local URL: http://localhost:8080"
    log "🌐 VM URL: http://$VM_IP:8080"
    log "📝 Logs: /tmp/firecracker-${VM_ID}.log"
    log "📝 VM Logs: /tmp/firecracker-${VM_ID}-vm.log"
    log ""
    log "Press Ctrl+C to stop the VM"

    # Keep script running
    wait $FC_PID
}

# Run main function
main "$@"