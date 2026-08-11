#!/usr/bin/env bash
set -Eeuo pipefail

repo=/home/fabio/services/wibble
state_dir=/var/lib/wibble-native-deploy
lock_file=/run/lock/wibble-native-deploy.lock

if [[ ${EUID} -ne 0 ]]; then
  echo "wibble updater must run as root" >&2
  exit 1
fi

exec 9>"${lock_file}"
flock -n 9 || exit 0
mkdir -p "${state_dir}"

run_as_fabio() {
  sudo -u fabio -H /bin/bash -lc "$1"
}

run_pm2() {
  sudo -u fabio -H env \
    PATH=/home/fabio/.local/bin:/home/fabio/.local/share/node-v22.19.0-linux-x64/bin:/usr/bin:/bin \
    /home/fabio/.local/bin/pm2 "$@"
}

run_as_fabio "git -C '${repo}' fetch --quiet origin master"
target_revision=$(run_as_fabio "git -C '${repo}' rev-parse origin/master")
deployed_revision=$(cat "${state_dir}/revision" 2>/dev/null || true)
[[ "${target_revision}" == "${deployed_revision}" ]] && exit 0

needs_install=false
config_only=false
if [[ -n "${deployed_revision}" ]] && ! run_as_fabio "git -C '${repo}' diff --quiet '${deployed_revision}' '${target_revision}' -- pnpm-lock.yaml package.json web/package.json worker/package.json"; then
  needs_install=true
fi
if [[ -n "${deployed_revision}" ]] \
  && run_as_fabio "git -C '${repo}' diff --quiet '${deployed_revision}' '${target_revision}' -- . ':(exclude)config.ncl'" \
  && ! run_as_fabio "git -C '${repo}' diff --quiet '${deployed_revision}' '${target_revision}' -- config.ncl"; then
  config_only=true
fi

if [[ -n $(run_as_fabio "git -C '${repo}' status --porcelain --untracked-files=no") ]]; then
  echo "wibble updater stopped: tracked worktree changes found" >&2
  exit 1
fi

backup_dir=$(mktemp -d /var/tmp/wibble-native-deploy.XXXXXX)
web_stopped=false
cleanup() {
  if [[ "${web_stopped}" == true ]]; then
    run_pm2 restart wibble-web
  fi
  case "${backup_dir}" in
    /var/tmp/wibble-native-deploy.*) rm -rf -- "${backup_dir}" ;;
  esac
}
trap cleanup EXIT

if [[ -d "${repo}/web/.next" ]]; then
  cp -a "${repo}/web/.next" "${backup_dir}/next"
fi

run_as_fabio "git -C '${repo}' pull --ff-only --quiet origin master"
/bin/bash "${repo}/deploy/install-nickel.sh"
run_as_fabio "/home/fabio/.local/bin/nickel export '${repo}/config.ncl' >/dev/null"
if [[ "${config_only}" == true ]]; then
  curl --fail --silent --show-error http://127.0.0.1:18001/api/runtime-config >/dev/null
  curl --fail --silent --show-error http://127.0.0.1:18002/health >/dev/null
  printf '%s\n' "${target_revision}" >"${state_dir}/revision"
  exit 0
fi
if [[ "${needs_install}" == true ]]; then
  run_as_fabio "cd '${repo}' && pnpm --config.minimum-release-age=10080 install --frozen-lockfile"
fi
for migration in "${repo}"/deploy/migrations/*.sql; do
  [[ -f "${migration}" ]] || continue
  sudo -u postgres psql -X -v ON_ERROR_STOP=1 -d wibble <"${migration}" >/dev/null
done
run_as_fabio "cd '${repo}' && pnpm --filter wibble-web exec prisma generate && pnpm --filter wibble-worker exec prisma generate"
run_pm2 stop wibble-web
web_stopped=true
if ! run_as_fabio "cd '${repo}' && pnpm typecheck && pnpm build"; then
  if [[ -d "${backup_dir}/next" ]]; then
    rm -rf -- "${repo}/web/.next"
    cp -a "${backup_dir}/next" "${repo}/web/.next"
    chown -R fabio:fabio "${repo}/web/.next"
  fi
  exit 1
fi

run_pm2 restart wibble-worker
run_pm2 restart wibble-web
web_stopped=false
curl --fail --silent --show-error --retry 12 --retry-connrefused --retry-delay 1 http://127.0.0.1:18001/ >/dev/null
curl --fail --silent --show-error --retry 12 --retry-connrefused --retry-delay 1 http://127.0.0.1:18002/health >/dev/null
printf '%s\n' "${target_revision}" >"${state_dir}/revision"
