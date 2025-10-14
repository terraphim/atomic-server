#!/bin/bash

# Atomic Server Production Monitoring Script
# This script monitors the health and status of Atomic Server deployment

set -euo pipefail

# Configuration
ATOMIC_PORT="8081"
PROXY_PORT="8082"
LOG_FILE="/tmp/atomic-server-monitor.log"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $1${NC}"
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] $1" >> "$LOG_FILE"
}

warn() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1${NC}"
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] WARNING: $1" >> "$LOG_FILE"
}

error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1${NC}"
    echo "[$(date +'%Y-%m-%d %H:%M:%S')] ERROR: $1" >> "$LOG_FILE"
}

# Check Atomic Server process
check_atomic_process() {
    log "Checking Atomic Server Docker container..."

    if docker ps --filter "name=atomic-server-production" --format "table {{.Names}}\t{{.Status}}" | grep -q "atomic-server-production"; then
        local container_info=$(docker inspect atomic-server-production)
        local status=$(echo "$container_info" | jq -r '.[0].State.Status')
        local health=$(echo "$container_info" | jq -r '.[0].State.Health.Status // "unknown"')

        echo "✅ Atomic Server container is running"
        echo "   Status: $status"
        echo "   Health: $health"

        # Get resource usage
        local stats=$(docker stats --no-stream --format "table {{.MemUsage}}\t{{.CPUPerc}}" atomic-server-production | tail -n 1)
        echo "   Resource usage: $stats"

        return 0
    else
        error "Atomic Server container is not running"
        return 1
    fi
}

# Check Caddy proxy process
check_caddy_process() {
    log "Checking Caddy proxy process..."

    if pgrep -f "caddy reverse-proxy.*--from :$PROXY_PORT" > /dev/null; then
        local pid=$(pgrep -f "caddy reverse-proxy.*--from :$PROXY_PORT")
        echo "✅ Caddy proxy is running (PID: $pid)"
        return 0
    else
        error "Caddy proxy is not running"
        return 1
    fi
}

# Check HTTP connectivity
check_http_connectivity() {
    log "Checking HTTP connectivity..."

    # Check Atomic Server directly
    if curl -s --connect-timeout 5 "http://localhost:$ATOMIC_PORT" > /dev/null; then
        echo "✅ Atomic Server (port $ATOMIC_PORT): responding"
    else
        error "Atomic Server (port $ATOMIC_PORT): not responding"
        return 1
    fi

    # Check Caddy proxy
    if curl -s --connect-timeout 5 "http://localhost:$PROXY_PORT" > /dev/null; then
        echo "✅ Caddy proxy (port $PROXY_PORT): responding"
    else
        error "Caddy proxy (port $PROXY_PORT): not responding"
        return 1
    fi
}

# Check database status
check_database() {
    log "Checking database status..."

    local db_file="/home/alex/infrastructure/atomic-server-turso/data/store.db"

    if [[ -f "$db_file" ]]; then
        local db_size=$(du -h "$db_file" | cut -f1)
        echo "✅ Database: ${db_size}"

        # Check if database is locked (indicates active writes)
        if [[ -f "${db_file}-wal" ]]; then
            local wal_size=$(du -h "${db_file}-wal" | cut -f1)
            echo "   WAL file: ${wal_size} (active)"
        else
            echo "   WAL file: not present (database idle)"
        fi

        # Check file permissions
        local db_owner=$(stat -c "%U:%G" "$db_file")
        echo "   Database owner: $db_owner"

        return 0
    else
        error "Database file not found at $db_file"
        return 1
    fi
}

# Check disk space
check_disk_space() {
    log "Checking disk space..."

    local df_output=$(df -h /tmp | tail -1)
    local available=$(echo "$df_output" | awk '{print $4}')
    local used_percent=$(echo "$df_output" | awk '{print $5}' | sed 's/%//')

    echo "Disk space: ${available} available (${used_percent}% used)"

    if [[ "$used_percent" -gt 90 ]]; then
        warn "Disk usage is high (${used_percent}%)"
        return 1
    else
        echo "✅ Disk space is adequate"
        return 0
    fi
}

# Check system resources
check_system_resources() {
    log "Checking system resources..."

    # Memory
    local mem_info=$(free -h | grep "Mem:")
    local total=$(echo "$mem_info" | awk '{print $2}')
    local used=$(echo "$mem_info" | awk '{print $3}')
    local available=$(echo "$mem_info" | awk '{print $7}')

    echo "Memory: ${used}/${total} (${available} available)"

    # Load average
    local load=$(uptime | grep -o "load average:.*" | cut -d: -f2)
    echo "Load average:$load"

    # Check for high load
    local load_1min=$(echo "$load" | awk '{print $1}' | sed 's/,//')
    if (( $(echo "$load_1min > 2.0" | bc -l) )); then
        warn "High system load: $load_1min"
        return 1
    else
        echo "✅ System load is normal"
        return 0
    fi
}

# Check recent logs for errors
check_logs() {
    log "Checking recent logs for errors..."

    # Check Atomic Server logs (if available)
    if pgrep -f "atomic-server" > /dev/null; then
        # This would require access to the application logs
        echo "ℹ️  Atomic Server logs: Check application log files"
    fi

    # Check system messages for recent errors
    local recent_errors=$(dmesg --ctime | tail -20 | grep -i "error\|fail\|panic" || echo "")
    if [[ -n "$recent_errors" ]]; then
        warn "Recent system errors found"
        echo "$recent_errors"
        return 1
    else
        echo "✅ No recent system errors"
        return 0
    fi
}

# Generate status report
generate_status_report() {
    log "Generating comprehensive status report..."

    echo "=================================================="
    echo "🔍 Atomic Server Production Status Report"
    echo "=================================================="
    echo "📅 Generated: $(date)"
    echo "🖥️  Host: $(hostname)"
    echo ""

    local overall_status=0

    check_atomic_process || overall_status=1
    echo ""
    check_caddy_process || overall_status=1
    echo ""
    check_http_connectivity || overall_status=1
    echo ""
    check_database || overall_status=1
    echo ""
    check_disk_space || overall_status=1
    echo ""
    check_system_resources || overall_status=1
    echo ""
    check_logs || overall_status=1
    echo ""

    echo "=================================================="
    if [[ "$overall_status" -eq 0 ]]; then
        echo "🟢 OVERALL STATUS: HEALTHY"
    else
        echo "🔴 OVERALL STATUS: ISSUES DETECTED"
    fi
    echo "=================================================="

    return "$overall_status"
}

# Main execution
main() {
    # Create log directory if it doesn't exist
    mkdir -p "$(dirname "$LOG_FILE")"

    # Generate status report
    generate_status_report
}

# Run main function
main "$@"