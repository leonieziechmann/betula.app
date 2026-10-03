#!/usr/bin/env bash
# ship.sh - build the images of this commit and bring an instance of the application to it.
# Runs on the workstation: Git Bash on Windows (Nix lives in a WSL distribution), or Linux with Nix.
#
#   SSH_TARGET=betula deploy/ship.sh canary --seed   # the FIRST deploy: with this machine's radix.db
#   SSH_TARGET=betula deploy/ship.sh canary          # every later one: build, load, sync, deploy, verify
#   deploy/ship.sh canary --build-only               # build and stop: nothing leaves this machine
#
# What happens, in this order:
#   1. The release is the commit HEAD (git archive), not the working tree: what runs on the server
#      can be looked up in git. Its tag is <date>-<short hash> of the last commit that changed what
#      the images are built from, so shipping the same sources twice changes nothing.
#   2. nix build .#radix-image .#folia-image (flake.nix). A release whose images are on the server
#      already is not built again.
#   3. Each image goes through ssh into "docker load" and gets the release tag there. No registry.
#   4. deploy/sync.sh, so that the stack files and scripts on the server belong to this release;
#      then deploy/ship-models.sh, which uploads the models of models.lock the server lacks.
#   5. Only on the first deploy of an instance, which has to say one of the two:
#      --seed     this machine's Radix database (SEED_DB) goes into the instance's volume first
#                 (vps/45-seed.sh), so the server does not crawl again what was crawled here.
#      --no-seed  Radix starts empty and crawls everything itself: hours, thousands of requests.
#   6. vps/50-app.sh <instance> <tag> on the server: checks images, DNS and secrets, deploys the
#      stack, waits until it has converged. Then vps/91-verify-stacks.sh services app.
#
# Blue-green (README.md section 4): an instance whose host another instance serves already
# (canary-green next to canary) is deployed as its standby, without the host's traffic;
# vps/55-switch.sh <instance> on the server hands the host over.
#
# An instance is a file stacks/<instance>.env (canary.env). Secrets are never part of this: the
# server has to know folia-access-password already (50-app.sh says how to create it).
# Rollback: ssh betula bash /opt/betula/vps/50-app.sh <instance> <previous tag>
#
# Environment:
#   SSH_TARGET, SSH_OPTS, DEPLOY_HOST, DEPLOY_USER   as in deploy/sync.sh
#   SHIP_WORKTREE=1   ship the working tree as it is (tracked or not, never ignored files)
#                     instead of HEAD. The tag gets "-wip-<time>", so it never passes for a commit.
#   SEED_DB           the database --seed uploads (default: radix.db in the repository's root).
#                     It has to fit the release: a database that a newer Radix has migrated is not
#                     for an older image - one more reason to ship the commit that made it.
#   WSL_DISTRO        the WSL distribution that has Nix (default NixOS); only read on Windows
#   SHIP_KEEP=1       keep the temporary directory with the image files
set -Eeuo pipefail

SCRIPT="$(basename "$0")"
DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${DEPLOY_DIR}/.." && pwd)"
# What the two images are built from (flake.nix; Radix also from Cortex's client, cortex/client).
# A change anywhere else is not a new release.
BUILD_PATHS=(flake.nix flake.lock folia/Cargo.toml folia/Cargo.lock folia/crates folia/assets radix/go.mod radix/go.sum radix/cmd radix/internal cortex/go.mod cortex/go.sum cortex/client)
IMAGES=(radix folia)

log() { printf '[%s] %s\n' "${SCRIPT}" "$*"; }
die() {
  printf '[%s] FATAL: %s\n' "${SCRIPT}" "$*" >&2
  exit 1
}
trap 'die "line ${LINENO}: \"${BASH_COMMAND}\" failed"' ERR

# instances -> the names that have a file deploy/stacks/<name>.env, on one line.
instances() {
  local file
  for file in "${DEPLOY_DIR}"/stacks/*.env; do
    [[ -f "${file}" ]] || continue
    file="${file##*/}"
    printf '%s ' "${file%.env}"
  done
}

INSTANCE="${1:-}"
BUILD_ONLY=0
SEED=""
case "${2:-}" in
  "") ;;
  --build-only) BUILD_ONLY=1 ;;
  --seed) SEED=yes ;;
  --no-seed) SEED=no ;;
  *) die "unknown option '${2}' (known: --seed, --no-seed, --build-only)" ;;
esac
[[ "$#" -le 2 && -n "${INSTANCE}" && "${INSTANCE}" != -* ]] || die "usage: ${SCRIPT} <instance> [--seed | --no-seed | --build-only]   (instances: $(instances))"
[[ -f "${DEPLOY_DIR}/stacks/${INSTANCE}.env" ]] || die "there is no instance '${INSTANCE}' (instances: $(instances)): deploy/stacks/${INSTANCE}.env does not exist"

DEPLOY_HOST="${DEPLOY_HOST:-betula.app}"
DEPLOY_USER="${DEPLOY_USER:-deploy}"
SSH_TARGET="${SSH_TARGET:-${DEPLOY_USER}@${DEPLOY_HOST}}"
[[ "${SSH_TARGET}" != -* ]] || die "SSH_TARGET must not start with '-'"
# Word splitting is the documented interface of SSH_OPTS; paths with spaces belong into ~/.ssh/config.
ssh_opts=()
if [[ -n "${SSH_OPTS:-}" ]]; then
  read -r -a ssh_opts <<<"${SSH_OPTS}"
fi
remote() { ssh ${ssh_opts[@]+"${ssh_opts[@]}"} "${SSH_TARGET}" "$@"; }

for tool in git tar gzip ssh stat; do
  command -v "${tool}" >/dev/null 2>&1 || die "required command not found: ${tool}"
done

# Said before anything is built: a first deploy that cannot be seeded should not cost a build first.
SEED_DB="${SEED_DB:-${REPO_DIR}/radix.db}"
if [[ "${SEED}" == "yes" ]]; then
  [[ -f "${SEED_DB}" ]] || die "--seed: there is no database at ${SEED_DB} (SEED_DB names another one)"
  [[ "$(basename "${SEED_DB}")" == "radix.db" ]] || die "SEED_DB has to be a file named radix.db: that is the name Radix opens in its volume"
fi
# state_of FILE... -> size and modification time of each (nothing for a file that does not exist).
state_of() {
  local file
  for file in "$@"; do
    [[ -e "${file}" ]] && stat -c '%n %s %Y' "${file}"
  done
  return 0
}

# Git Bash rewrites arguments that look like POSIX paths when it starts a native Windows program
# (ssh.exe, wsl.exe): "/opt/betula" would become "C:/Program Files/Git/opt/betula".
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'
# macOS tar would add "._*" resource-fork files.
export COPYFILE_DISABLE=1

cd "${REPO_DIR}"

# ---------------------------------------------------------------- the release

# The last commit that changed what the images are built from: a commit to the docs or to
# deploy/ is not a new release, and must not restart the services under a new tag.
commit="$(git log -1 --format=%h --abbrev=7 HEAD -- "${BUILD_PATHS[@]}")"
day="$(git log -1 --format=%cd --date=format:%Y-%m-%d HEAD -- "${BUILD_PATHS[@]}")"
[[ -n "${commit}" && -n "${day}" ]] || die "no commit touches ${BUILD_PATHS[*]}: is this the repository?"
if [[ "${SHIP_WORKTREE:-0}" == "1" ]]; then
  TAG="${day}-${commit}-wip-$(date +%H%M%S)"
  log "SHIP_WORKTREE=1: shipping the working tree as it is, as ${TAG}"
else
  TAG="${day}-${commit}"
  dirty="$(git status --porcelain -- "${BUILD_PATHS[@]}")"
  if [[ -n "${dirty}" ]]; then
    printf '%s\n' "${dirty}" >&2
    die "the changes above are not committed, and a release is a commit. Commit them, or ship the working tree as it is with SHIP_WORKTREE=1"
  fi
fi

# Impressum and Datenschutz are final while folia/crates/app/src/pages/legal.rs says PLACEHOLDER = false (the
# real texts since 2026-09-25). An instance open to everybody (FOLIA_ACCESS_GATE other than "on")
# must not go out without them (owner, 2026-09-21: placeholders first, "aber so, dass wir das
# nicht vergessen"); behind the gate it may.
gate="$(sed -n 's/^FOLIA_ACCESS_GATE=//p' "${DEPLOY_DIR}/stacks/${INSTANCE}.env" | tr -d '\r' | tail -n 1)"
if [[ "${gate}" != "on" ]]; then
  if [[ "${SHIP_WORKTREE:-0}" == "1" ]]; then
    legal="$(cat folia/crates/app/src/pages/legal.rs 2>/dev/null)" || legal=""
  else
    legal="$(git show HEAD:folia/crates/app/src/pages/legal.rs 2>/dev/null)" || legal=""
  fi
  [[ -n "${legal}" ]] || die "instance '${INSTANCE}' is open to everybody (FOLIA_ACCESS_GATE=${gate:-unset}), but this release has no folia/crates/app/src/pages/legal.rs: no Impressum, no Datenschutz."
  if grep -q '^pub const PLACEHOLDER: bool = true;' <<<"${legal}"; then
    die "instance '${INSTANCE}' is open to everybody (FOLIA_ACCESS_GATE=${gate:-unset}), but Impressum and Datenschutz are placeholders still (folia/crates/app/src/pages/legal.rs: PLACEHOLDER = true). Write the real texts and set it to false, or keep the gate on."
  fi
fi
log "release ${TAG} for instance ${INSTANCE}"

# ---------------------------------------------------------------- where Nix runs

# On Windows, Nix runs inside WSL and files travel through a directory both sides can see.
WSL=()
if ! command -v nix >/dev/null 2>&1; then
  command -v wsl.exe >/dev/null 2>&1 || die "nix is not on the PATH (and there is no wsl.exe to look for it in a WSL distribution)"
  WSL=(wsl.exe -d "${WSL_DISTRO:-NixOS}" --)
  "${WSL[@]}" bash -c 'command -v nix' >/dev/null 2>&1 || die "no nix inside the WSL distribution '${WSL_DISTRO:-NixOS}' (WSL_DISTRO names another one)"
fi
# run_nix_side COMMAND... - on this machine, or inside the WSL distribution.
# (the "+" form: an empty array is an "unbound variable" for older bash versions)
run_nix_side() { ${WSL[@]+"${WSL[@]}"} "$@"; }

WORK="$(mktemp -d)"
# native PATH -> the path as a native Windows program has to be told it. Git Bash converts no
# arguments here (MSYS_NO_PATHCONV above), so git.exe would not find "/tmp/..."; and the tools of
# Git Bash itself must keep the POSIX form, because GNU tar takes "C:/..." for a remote host.
native() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$1"
  else
    printf '%s' "$1"
  fi
}
cleanup() {
  if [[ "${SHIP_KEEP:-0}" == "1" || "${BUILD_ONLY}" == "1" ]]; then
    log "the image files stay in ${WORK}"
  else
    rm -rf -- "${WORK}"
  fi
}
trap cleanup EXIT

# inside PATH -> the same path as the side that runs Nix sees it.
inside() {
  if [[ "${#WSL[@]}" -eq 0 ]]; then
    printf '%s' "$1"
  else
    run_nix_side wslpath -u "$(cygpath -m "$1")"
  fi
}

# ---------------------------------------------------------------- build

needs_build=1
if [[ "${BUILD_ONLY}" == "0" && "${SHIP_WORKTREE:-0}" != "1" ]]; then
  # One connection, one line: which of the two images of this release are loaded already.
  loaded="$(remote "for i in ${IMAGES[*]}; do docker image inspect betula-\$i:${TAG} >/dev/null 2>&1 && printf '%s ' \$i; done" || true)"
  if [[ "${loaded}" == "${IMAGES[*]} " ]]; then
    log "the images of ${TAG} are on the server already: nothing to build"
    needs_build=0
  fi
fi

if [[ "${needs_build}" == "1" ]]; then
  log "exporting the sources"
  tree="HEAD"
  if [[ "${SHIP_WORKTREE:-0}" == "1" ]]; then
    # The working tree as a tree object, made with an index of its own: what git would commit
    # (line endings included), tracked or not, never ignored files. The real index, HEAD and the
    # working files are not touched.
    tree="$(
      GIT_INDEX_FILE="$(native "${WORK}/index")"
      export GIT_INDEX_FILE
      git read-tree HEAD
      git add -A -- "${BUILD_PATHS[@]}"
      git write-tree
    )"
  fi
  # As the repository stores them, byte for byte: the two settings pin the export to LF whatever
  # this machine's checkout does. Seen on Windows with core.autocrlf=true (2026-09-21): an export
  # without them carried CRLF in 168 files, and the carriage returns inside flake.nix ended up in
  # the shell scripts of the build: "$'\r': command not found".
  git -c core.autocrlf=false -c core.eol=lf archive --format=tar.gz -o "$(native "${WORK}/src.tar.gz")" "${tree}" -- "${BUILD_PATHS[@]}"

  # Runs where Nix is. The sources are unpacked into that side's own file system (a build on
  # /mnt/c would crawl), built as a "path:" flake (no git needed there), and the two image files
  # (docker archives, gzip) are copied back next to the script.
  cat >"${WORK}/build.sh" <<'BUILD'
#!/usr/bin/env bash
set -Eeuo pipefail
work="$1"
src="${XDG_CACHE_HOME:-$HOME/.cache}/betula-ship/src"
rm -rf -- "${src}"
mkdir -p "${src}"
tar -xzf "${work}/src.tar.gz" -C "${src}"
for image in radix folia; do
  echo "[build] nix build .#${image}-image"
  nix --extra-experimental-features 'nix-command flakes' build "path:${src}#${image}-image" --out-link "${src}/../result-${image}" --print-build-logs
  cp -fL "${src}/../result-${image}" "${work}/${image}-image.tar.gz"
done
BUILD
  log "building the images with Nix (the first build of a release takes a while)"
  run_nix_side bash "$(inside "${WORK}/build.sh")" "$(inside "${WORK}")"
  for image in "${IMAGES[@]}"; do
    [[ -s "${WORK}/${image}-image.tar.gz" ]] || die "the build left no ${image}-image.tar.gz"
    log "built: ${image}-image.tar.gz ($(du -h "${WORK}/${image}-image.tar.gz" | cut -f1))"
  done
fi

if [[ "${BUILD_ONLY}" == "1" ]]; then
  log "--build-only: nothing was sent anywhere. Try an image: docker load < ${WORK}/folia-image.tar.gz"
  exit 0
fi

# ---------------------------------------------------------------- load, sync, deploy

if [[ "${needs_build}" == "1" ]]; then
  for image in "${IMAGES[@]}"; do
    log "loading betula-${image}:${TAG} on ${SSH_TARGET}"
    # The flake names every build "latest"; the release tag is given where it is loaded. The
    # name "latest" is never deployed (50-app.sh refuses it).
    remote "docker load -q && docker tag betula-${image}:latest betula-${image}:${TAG}" <"${WORK}/${image}-image.tar.gz"
  done
fi

log "syncing deploy/ (stack files and scripts of this release)"
SSH_TARGET="${SSH_TARGET}" SSH_OPTS="${SSH_OPTS:-}" bash "${DEPLOY_DIR}/sync.sh"

# The models of the semantic search (models.lock), when the server's store lacks them. Not a
# reason to stop: without them 50-app.sh deploys the instance without the semantic search, and says so.
if ! SHIP_MODELS_SYNC=0 SSH_TARGET="${SSH_TARGET}" SSH_OPTS="${SSH_OPTS:-}" bash "${DEPLOY_DIR}/ship-models.sh"; then
  log "WARNING: the models of models.lock are not on the server (above: why); ${INSTANCE} runs without the semantic search until deploy/ship-models.sh has brought them and it is deployed again"
fi

# ---------------------------------------------------------------- the first deploy: data

# An instance that has no volume for Radix yet has never been deployed.
first_deploy="$(remote "docker volume inspect ${INSTANCE}_radix-data >/dev/null 2>&1 && echo no || echo yes")"
if [[ "${first_deploy}" == "yes" && -z "${SEED}" ]]; then
  die "this is the first deploy of ${INSTANCE}, so say where its data comes from: --seed uploads ${SEED_DB} (nothing is crawled twice), --no-seed lets the server crawl everything itself (hours, and thousands of requests to the university's servers). The images are loaded; the next run does not build them again"
fi
if [[ "${SEED}" == "yes" ]]; then
  # A copy first: what is uploaded must not change while it is read. SQLite keeps what it has
  # not yet moved into the database in "<db>-wal"; an empty one has nothing to add.
  before="$(state_of "${SEED_DB}" "${SEED_DB}-wal")"
  files=("$(basename "${SEED_DB}")")
  if [[ -s "${SEED_DB}-wal" ]]; then
    files+=("$(basename "${SEED_DB}")-wal")
  fi
  log "packing ${files[*]} ($(du -h "${SEED_DB}" | cut -f1))"
  # As root's files: Radix runs as root without capabilities, and a file of another user would
  # be read-only to it (45-seed.sh makes sure of the same on its side).
  tar -C "$(dirname "${SEED_DB}")" --owner=0 --group=0 --mode=0644 -cf - "${files[@]}" | gzip -1 >"${WORK}/seed.tar.gz"
  [[ "$(state_of "${SEED_DB}" "${SEED_DB}-wal")" == "${before}" ]] ||
    die "${SEED_DB} changed while it was being read: a Radix is writing to it. Stop it (or wait for its crawl to end) and run this again"
  log "seeding ${INSTANCE} with it ($(du -h "${WORK}/seed.tar.gz" | cut -f1) to upload)"
  remote "bash /opt/betula/vps/45-seed.sh ${INSTANCE} ${TAG} $(stat -c '%s' "${SEED_DB}")" <"${WORK}/seed.tar.gz"
fi

log "deploying instance ${INSTANCE}"
remote "bash /opt/betula/vps/50-app.sh ${INSTANCE} ${TAG}"
remote "bash /opt/betula/vps/91-verify-stacks.sh services app"
log "done: instance ${INSTANCE} runs ${TAG}"
