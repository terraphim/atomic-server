#!/bin/bash

# Stop Atomic Server Firecracker VM
# This script stops a running Atomic Server Firecracker VM

set -euo pipefail

# Configuration
VM_ID="${1:-}"
VM_IP="169.254.100.2"
TAP_IP="169.254.100.1"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
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

# Stop VM by ID or find all Atomic Server VMs
stop_vm() {
    local vm_to_stop="$1"

    if [[ -n "$vm_to_stop" ]]; then
        # Stop specific VM
        log "Stopping Atomic Server VM: $vm_to_stop"

        # Kill Firecracker process
        if pgrep -f "firecracker.*--id.*$vm_to_stop" > /dev/null; then
            pkill -f "firecracker.*--id.*$vm_to_stop" || true
            log "Stopped Firecracker process for VM: $vm_to_stop"
        fi

        # Remove API socket
        rm -f "/tmp/firecracker-${vm_to_stop}.sock"

        # Remove TAP device
        local tap_dev="fc-${vm_to_stop}"
        if ip link show "$tap_dev" &>/dev/null; then
            ip link del "$tap_dev"
            log "Removed TAP device: $tap_dev"
        fi

        # Remove iptables rules
        iptables -t nat -D PREROUTING -p tcp --dport 8080 -j DNAT --to-destination $VM_IP:8080 2>/dev/null || true
        iptables -t nat -D POSTROUTING -s $VM_IP -j MASQUERADE 2>/dev/null || true
        iptables -D FORWARD -d $VM_IP -p tcp --dport 8080 -j ACCEPT 2>/dev/null || true

    else
        # Stop all Atomic Server VMs
        log "Stopping all Atomic Server Firecracker VMs..."

        # Find all Firecracker processes
        local pids=$(pgrep -f "firecracker" || true)
        if [[ -n "$pids" ]]; then
            echo "$pids" | xargs kill 2>/dev/null || true
            log "Stopped all Firecracker processes"
        fi

        # Remove all API sockets
        rm -f /tmp/firecracker-*.sock

        # Remove all TAP devices
        for tap in $(ip link show | grep -o "fc-[^:]*" || true); do
            ip link del "$tap" 2>/dev/null || true
        done
        log "Removed all TAP devices"

        # Clean up iptables rules
        iptables -t nat -F PREROUTING 2>/dev/null || true
        iptables -t nat -F POSTROUTING 2>/dev/null || true
        iptables -F FORWARD 2>/dev/null || true
    fi
}

# Cleanup logs and temporary files
cleanup() {
    log "Cleaning up temporary files..."

    if [[ -n "$1" ]]; then
        # Clean specific VM
        rm -f "/tmp/firecracker-${1}.log" "/tmp/firecracker-${1}-vm.log"
    else
        # Clean all
        rm -f /tmp/firecracker-*.log
    fi

    log "Cleanup complete."
}

# Main execution
main() {
    log "Atomic Server Firecracker VM shutdown..."

    if [[ -n "$1" ]]; then
        stop_vm "$1"
        cleanup "$1"
        log "Atomic Server VM '$1' stopped successfully."
    else
        stop_vm
        cleanup
        log "All Atomic Server VMs stopped successfully."
    fi
}

# Run main function
main "$@"