#!/usr/bin/env bash
# Deploy 保卫萝卜 web build to https://protect.gz.autolife.ai:8444/
#
# Transfer-optimized for gz-office's slow inbound link:
#   - ship ONLY the brotli-compressed wasm/js (.br, ~33% smaller than .gz)
#   - regenerate raw + .gz server-side (gz has the `brotli` CLI)
#   - relay through the jump host (my->jump is fast; jump->gz is the slow hop,
#     so it runs detached with a resumable rsync loop)
#
# nginx on gz serves .br (brotli_static) -> .gz (gzip_static) -> raw by
# Accept-Encoding. The raw file must exist for `try_files` to match.
#
# Secrets come from the environment — never hard-code them:
#   JUMP_PW  password for the jump host   (ubuntu@43.156.66.157)
#   GZ_PW    password for gz-office       (autolife@183.6.107.47)
# Usage:  JUMP_PW=... GZ_PW=... ./deploy.sh
set -euo pipefail

: "${JUMP_PW:?set JUMP_PW (jump host password)}"
: "${GZ_PW:?set GZ_PW (gz-office password)}"

PROJ="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NAME=protect_carrot
JUMP=ubuntu@43.156.66.157
GZ=autolife@183.6.107.47
GZP=2222
URL=https://protect.gz.autolife.ai:8444
O="-o PreferredAuthentications=password -o PubkeyAuthentication=no -o IdentityAgent=none -o StrictHostKeyChecking=accept-new -o ConnectTimeout=25"

cd "$PROJ"

echo "==> 1/6 build (release, WebGPU + brotli)"
nix develop --command ./build-web.sh release

echo "==> 2/6 stage tar (.br only for wasm/js; raw+gz regenerated on gz)"
mkdir -p .deploy
tar -C web \
  --exclude="${NAME}_bg.wasm" \
  --exclude="${NAME}_bg.wasm.gz" \
  --exclude="${NAME}.js.gz" \
  -cf .deploy/protect-update.tar .
echo "    tar: $(stat -c%s .deploy/protect-update.tar) bytes"

echo "==> 3/6 push to jump host"
SSHPASS="$JUMP_PW" sshpass -e rsync -e "ssh $O" --inplace \
  .deploy/protect-update.tar "$JUMP:/tmp/protect-update.tar"

echo "==> 4/6 launch detached resumable relay jump->gz"
SSHPASS="$JUMP_PW" sshpass -e ssh $O "$JUMP" "bash -s" <<RELAYLAUNCH
export GZ_PW='$GZ_PW'
cat > /tmp/relay.sh <<'SCRIPT'
#!/bin/bash
export SSHPASS="\$GZ_PW"
O="-o PreferredAuthentications=password -o PubkeyAuthentication=no -o IdentityAgent=none -o StrictHostKeyChecking=accept-new -o ConnectTimeout=20 -p $GZP"
sshpass -e ssh \$O -n $GZ "rm -f /home/autolife/protect-update.tar"
n=0
until [ \$n -ge 40 ]; do
  n=\$((n+1)); echo ">>> attempt \$n \$(date +%H:%M:%S)"
  if sshpass -e rsync -e "ssh \$O" --inplace --partial --append-verify --timeout=120 /tmp/protect-update.tar $GZ:/home/autolife/protect-update.tar; then
    echo RELAY_OK; sshpass -e ssh \$O -n $GZ "md5sum /home/autolife/protect-update.tar"; break
  fi
  echo "  retry 5s"; sleep 5
done
SCRIPT
chmod +x /tmp/relay.sh
nohup bash /tmp/relay.sh > /tmp/relay.log 2>&1 &
echo "relay pid \$!"
RELAYLAUNCH

echo "==> 5/6 wait for RELAY_OK (slow link; minutes). Polling jump log only."
until SSHPASS="$JUMP_PW" sshpass -e ssh $O "$JUMP" 'grep -q RELAY_OK /tmp/relay.log' 2>/dev/null; do
  sleep 20
done
echo "    RELAY_OK"

echo "==> 6/6 apply on gz (regenerate raw+gz from .br, rsync --delete)"
SSHPASS="$GZ_PW" sshpass -e ssh $O -p "$GZP" "$GZ" "bash -s" <<APPLY
set -e
cd /home/autolife
rm -rf protect-deploy && mkdir -p protect-deploy
tar -xf protect-update.tar -C protect-deploy
cd protect-deploy
brotli -d -k -f ${NAME}_bg.wasm.br      # raw wasm (needed for try_files)
gzip  -9 -k -f ${NAME}_bg.wasm          # gzip fallback
gzip  -9 -k -f ${NAME}.js
cd /home/autolife
printf '#!/bin/sh\necho "%s"\n' '$GZ_PW' > /tmp/ap.sh
chmod +x /tmp/ap.sh; export SUDO_ASKPASS=/tmp/ap.sh
sudo -A rsync -a --delete protect-deploy/ /var/www/protect/
sudo -A chown -R root:root /var/www/protect
sudo -A find /var/www/protect -type d -exec chmod 755 {} +
sudo -A find /var/www/protect -type f -exec chmod 644 {} +
rm -f /tmp/ap.sh; rm -rf protect-deploy protect-update.tar
echo "applied; live wasm.br md5: \$(md5sum /var/www/protect/${NAME}_bg.wasm.br | cut -d' ' -f1)"
APPLY

echo "==> verify"
for enc in br gzip; do
  line=$(curl -k -s -H "Accept-Encoding: $enc" -o /dev/null -D - "$URL/${NAME}_bg.wasm")
  code=$(curl -k -s -H "Accept-Encoding: $enc" -o /dev/null -w '%{http_code}' "$URL/${NAME}_bg.wasm")
  ce=$(printf '%s' "$line" | awk 'tolower($1)=="content-encoding:"{print $2}' | tr -d '\r')
  cl=$(printf '%s' "$line" | awk 'tolower($1)=="content-length:"{print $2}' | tr -d '\r')
  echo "    wasm [$enc]: http=$code content-encoding=${ce:-none} length=${cl:-?}"
done

echo "==> cleanup"
SSHPASS="$JUMP_PW" sshpass -e ssh $O "$JUMP" 'rm -f /tmp/relay.sh /tmp/relay.log /tmp/protect-update.tar' || true
rm -rf "$PROJ/.deploy"
echo "==> done: $URL"
