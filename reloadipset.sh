#!/bin/bash
set -euo pipefail

mkdir -p /tmp/redblock
cd /tmp/redblock

wget -q https://cdn.dispherical.com/redblock/list.txt -O list.txt
grep -v '^#' list.txt | grep -v '^[[:space:]]*$' > clean-list.txt

echo "[Redblock] Building ipset restore file..."
{
  echo "create blocked4 hash:net family inet hashsize 65536 maxelem 10000000"
  awk '/\./{print "add -exist blocked4", $0}' clean-list.txt
  echo "create blocked6 hash:net family inet6 hashsize 65536 maxelem 10000000"
  awk -F: '{if (NF>1) print "add -exist blocked6", $0}' clean-list.txt
} > ipset-restore.txt

sudo ipset destroy blocked4 2>/dev/null || true
sudo ipset destroy blocked6 2>/dev/null || true
sudo ipset restore < ipset-restore.txt
