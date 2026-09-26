#!/bin/sh
# Versioned, read-only Linux host probe. Keep dependencies to POSIX shell,
# procfs, sysfs, awk, uname, hostname, and df.

printf 'version=1\n'
printf 'hostname=%s\n' "$(hostname 2>/dev/null || printf unknown)"
printf 'kernel=%s\n' "$(uname -r 2>/dev/null || printf unknown)"

os=Linux
if [ -r /etc/os-release ]; then
    os=$(awk -F= '/^PRETTY_NAME=/{sub(/^"/, "", $2); sub(/"$/, "", $2); print $2; exit}' /etc/os-release)
fi
printf 'os=%s\n' "${os:-Linux}"

awk '/^cpu / {
    total=0
    for (i=2; i<=NF; i++) total += $i
    idle=$5+$6
    printf "cpu_total_ticks=%.0f\ncpu_idle_ticks=%.0f\n", total, idle
    exit
}' /proc/stat

awk '
    /^MemTotal:/ { total=$2*1024 }
    /^MemAvailable:/ { available=$2*1024 }
    END {
        printf "mem_total_bytes=%.0f\nmem_available_bytes=%.0f\n", total, available
    }
' /proc/meminfo

awk '{ printf "load_1=%s\nload_5=%s\nload_15=%s\n", $1, $2, $3 }' /proc/loadavg
awk '{ printf "uptime_secs=%.0f\n", $1 }' /proc/uptime

df -Pk / | awk 'NR==2 {
    printf "root_total_bytes=%.0f\nroot_available_bytes=%.0f\n", $2*1024, $4*1024
}'

awk '
    BEGIN { rx=0; tx=0 }
    FNR==1 {
        path=FILENAME
        sub("/statistics/.*", "", path)
        count=split(path, parts, "/")
        iface=parts[count]
    }
    iface != "lo" && FILENAME ~ /rx_bytes$/ { rx += $1 }
    iface != "lo" && FILENAME ~ /tx_bytes$/ { tx += $1 }
    END {
        printf "network_rx_bytes=%.0f\nnetwork_tx_bytes=%.0f\n", rx, tx
    }
' /sys/class/net/*/statistics/rx_bytes /sys/class/net/*/statistics/tx_bytes 2>/dev/null
