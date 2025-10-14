#!/bin/bash

# Atomic Server Firecracker VM Status Monitor
# This script checks the status of running Atomic Server VMs

set -euo pipefail

# Configuration
VM_IP="169.254.100.2"
FC_PORT="8080"

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
}

# Check if Firecracker is running
check_firecracker() {
    log "Checking Firecracker processes..."

    local fc_processes=$(pgrep -f "firecracker" || echo "none")
    echo "Firecracker PIDs: $fc_processes"

    if [[ "$fc_processes" != "none" ]]; then
        echo "$fc_processes" | while read pid; do
            if [[ -n "$pid" ]]; then
                local cmd=$(ps -p "$pid" -o args --no-headers 2>/dev/null || echo "process not found")
                echo "  PID $pid: $cmd"
            fi
        done
    fi
}

# Check network interfaces
check_network() {
    log "Checking network interfaces..."

    local tap_devices=$(ip link show | grep -o "fc-[^:]*" || echo "none")
    echo "TAP devices: $tap_devices"

    if [[ "$tap_devices" != "none" ]]; then
        echo "$tap_devices" | while read tap; do
            if [[ -n "$tap" ]]; then
                local status=$(ip link show "$tap" | grep -o "state [A-Z]*" | cut -d' ' -f2)
                local ip=$(ip addr show "$tap" | grep -o "inet [0-9.]*/" | cut -d' ' -f2)
                echo "  $tap: state=$status, ip=$ip"
            fi
        done
    fi
}

# Check VM connectivity
check_vm_connectivity() {
    log "Checking VM connectivity..."

    if curl -s --connect-timeout 2 "http://$VM_IP:$FC_PORT" >/dev/null 2>&1; then
        echo "✅ Atomic Server is responding at http://$VM_IP:$FC_PORT"

        # Get server info
        local version=$(curl -s "http://$VM_IP:$FC_PORT/api/v1/server/version" 2>/dev/null || echo "version not available")
        echo "  Server version: $version"

        # Check database status
        local db_status=$(curl -s "http://$VM_IP:$FC_PORT/api/v1/server/health" 2>/dev/null || echo "health check failed")
        echo "  Health status: $db_status"
    else
        echo "❌ Atomic Server is not responding at http://$VM_IP:$FC_PORT"
    fi

    # Check local port forwarding
    if curl -s --connect-timeout 2 "http://localhost:$FC_PORT" >/dev/null 2>&1; then
        echo "✅ Local port forwarding is working at http://localhost:$FC_PORT"
    else
        echo "❌ Local port forwarding is not working at http://localhost:$FC_PORT"
    fi
}

# Check system resources
check_resources() {
    log "Checking system resources..."

    # Memory usage
    local mem_info=$(free -h | grep "Mem:")
    echo "Memory: $mem_info"

    # Disk space
    local disk_info=$(df -h /tmp | tail -1)
    echo "Disk (/tmp): $disk_info"

    # Load average
    local load=$(uptime | grep -o "load average:.*" | cut -d: -f2)
    echo "Load average:$load"
}

# Check logs for errors
check_logs() {
    log "Checking recent logs for errors..."

    local log_files=("/tmp/firecracker-"*.log)
    local found_errors=false

    for log_file in "${log_files[@]}"; do
        if [[ -f "$log_file" ]]; then
            local recent_errors=$(tail -50 "$log_file" 2>/dev/null | grep -i "error\|fail\|panic" || echo "")
            if [[ -n "$recent_errors" ]]; then
                echo "❌ Errors found in $(basename "$log_file"):"
                echo "$recent_errors"
                found_errors=true
            fi
        fi
    done

    if [[ "$found_errors" == false ]]; then
        echo "✅ No recent errors found in logs"
    fi
}

# Check iptables rules
check_iptables() {
    log "Checking iptables rules..."

    local nat_prerouting=$(iptables -t nat -L PREROUTING -n --line-numbers 2>/dev/null | grep "8080" || echo "none")
    local nat_postrouting=$(iptables -t nat -L POSTROUTING -n --line-numbers 2>/dev/null | grep "MASQUERADE" || echo "none")
    local forward_rules=$(iptables -L FORWARD -n --line-numbers 2>/dev/null | grep "8080" || echo "none")

    echo "NAT PREROUTING rules for port 8080: $nat_prerouting"
    echo "NAT POSTROUTING MASQUERADE rules: $nat_postrouting"
    echo "FORWARD rules for port 8080: $forward_rules"
}

# Main status check
main() {
    echo "=================================================="
    echo "🔍 Atomic Server Firecracker VM Status Report"
    echo "=================================================="
    echo

    check_firecracker
    echo

    check_network
    echo

    check_vm_connectivity
    echo

    check_resources
    echo

    check_iptables
    echo

    check_logs
    echo

    echo "=================================================="
    echo "📊 Status check completed at $(date)"
    echo "=================================================="
}

# Show specific VM status if ID provided
if [[ -n "${1:-}" ]]; then
    VM_ID="$1"
    echo "Checking specific VM: $VM_ID"
    API_SOCKET="/tmp/firecracker-${VM_ID}.sock"

    if [[ -e "$API_SOCKET" ]]; then
        log "VM $VM_ID API socket found"

        # Check VM info via API
        local vm_info=$(curl -s --unix-socket "$API_SOCKET" http://localhost/machine-config 2>/dev/null || echo "API call failed")
        echo "VM Config: $vm_info"

        # Check if VM is running
        local instance_info=$(curl -s --unix-socket "$API_SOCKET" http://localhost/info 2>/dev/null || echo "Instance info not available")
        echo "Instance Info: $instance_info"
    else
        warn "VM $VM_ID API socket not found"
    fi
else
    main
fi