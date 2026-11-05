#!/usr/bin/env bash
# ship-phloem.sh - build the Phloem image of this commit and bring an instance of Phloem, the
# service of Betula Sync (stacks/phloem.yml, docs/phloem/phloem.md), to it. Runs on the
# workstation, as deploy/ship-cortex.sh does: Git Bash on Windows (Nix lives in a WSL
# distribution), or Linux with Nix.
#
#   SSH_TARGET=betula deploy/ship-phloem.sh phloem-canary    # build, load, sync, deploy
#   SSH_TARGET=betula deploy/ship-phloem.sh phloem           # the same for https://betula.app
#   deploy/ship-phloem.sh --build-only                       # build and stop: nothing leaves this machine
#
# What happens, in this order:
#   1. The release is the commit HEAD (git archive), not the working tree. Its tag is <date>-<short
#      hash> of the last commit that changed what the image is built from: the paths phloemPaths
#      names in flake.nix (the Go module phloem/ and its firewall), flake.nix and flake.lock. A
#      commit to Radix, Folia, Cortex, the docs or deploy/ is no new Phloem.
#   2. nix build .#phloem-image (flake.nix). A release whose image is on the server already is
#      not built again.
#   3. The image goes through ssh into "docker load" and gets the release tag there. No registry.
#   4. deploy/sync.sh, so that the stack file, the instance's file (stacks/phloem/<instance>.env)
#      and the scripts on the server belong to this release.
#   5. vps/49-phloem.sh <instance> <tag> on the server: the deploy, and the checks that try what is
#      forbidden afterwards. A FAIL there ends this script with its exit code.
#
# Each instance goes on its own: phloem-canary first, then phloem. Phloem is not part of the
# canary pipeline (.github/workflows/images.yml builds Radix and Folia): a push to master does not
# restart the service that holds the profiles of betula.app. Before the first deploy of an
# instance, the server needs its file systems: sudo bash /opt/betula/vps/47-phloem-host.sh
# (49-phloem.sh refuses to deploy without them, and says so).
# Rollback: ssh betula bash /opt/betula/vps/49-phloem.sh <instance> <previous tag>
#
# Environment:
#   SSH_TARGET, SSH_OPTS, DEPLOY_HOST, DEPLOY_USER   as in deploy/sync.sh
#   SHIP_WORKTREE=1   ship the working tree as it is (tracked or not, never ignored files)
#                     instead of HEAD. The tag gets "-wip-<time>", so it never passes for a commit.
#   WSL_DISTRO        the WSL distribution that has Nix (default NixOS); only read on Windows
#   SHIP_KEEP=1       keep the temporary directory with the image file
set -Eeuo pipefail

SCRIPT="$(basename "$0")"
DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${DEPLOY_DIR}/.." && pwd)"

log() { printf '[%s] %s\n' "${SCRIPT}" "$*"; }
die() {
  printf '[%s] FATAL: %s\n' "${SCRIPT}" "$*" >&2
  exit 1
}
trap 'die "line ${LINENO}: \"${BASH_COMMAND}\" failed"' ERR

usage() {
  local file names=""
  for file in "${DEPLOY_DIR}"/stacks/phloem/*.env; do
    [[ -f "${file}" ]] || continue
    file="${file##*/}"
    names+="${file%.env} "
  done
  die "usage: ${SCRIPT} <instance> | --build-only   (instances: ${names% })"
}

BUILD_ONLY=0
INSTANCE=""
case "${1:-}" in
  "") usage ;;
  --build-only) BUILD_ONLY=1 ;;
  -*) usage ;;
  *) INSTANCE="$1" ;;
esac
[[ "$#" -eq 1 ]] || usage
if [[ "${BUILD_ONLY}" == "0" ]]; then
  # The same rule as on the server (vps/lib-phloem.sh, load_phloem), checked here before anything
  # is built: the name is a stack, a network and a router.
  [[ "${INSTANCE}" =~ ^phloem(-[a-z0-9]+)*$ ]] || usage
  [[ -f "${DEPLOY_DIR}/stacks/phloem/${INSTANCE}.env" ]] ||
    die "there is no instance '${INSTANCE}': deploy/stacks/phloem/${INSTANCE}.env does not exist"
fi

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

for tool in git tar gzip ssh sed; do
  command -v "${tool}" >/dev/null 2>&1 || die "required command not found: ${tool}"
done

# Git Bash rewrites arguments that look like POSIX paths when it starts a native Windows program
# (ssh.exe, wsl.exe): "/opt/betula" would become "C:/Program Files/Git/opt/betula".
export MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'
# macOS tar would add "._*" resource-fork files.
export COPYFILE_DISABLE=1

cd "${REPO_DIR}"

# ---------------------------------------------------------------- the release

# What the image is built from, and so what its tag is made of: the one line "phloemPaths = [ ...
# ];" of flake.nix (the binary's source and its firewall) and the flake itself. Read, not
# repeated here, so that the tag and the build cannot come apart.
line="$(sed -n -E 's/^[[:space:]]*phloemPaths = \[(.*)\];[[:space:]]*$/\1/p' flake.nix | tr -d '\r')"
read -r -a TAG_PATHS <<<"${line//\"/}"
[[ "${#TAG_PATHS[@]}" -gt 0 ]] || die "flake.nix has no line 'phloemPaths = [ ... ];' that says what the image is built from"
for path in "${TAG_PATHS[@]}"; do
  [[ "${path}" =~ ^[A-Za-z0-9][A-Za-z0-9_./-]*$ && "${path}" != *..* ]] || die "flake.nix: phloemPaths names '${path}', which is not a path of the Go module phloem/"
done
# phloemPaths are relative to the Go module phloem/; git wants them from the repository's root.
TAG_PATHS=("${TAG_PATHS[@]/#/phloem/}")
# What the build gets: the same, and the flake.
EXPORT_PATHS=(flake.nix flake.lock "${TAG_PATHS[@]}")
TAG_PATHS+=(flake.nix flake.lock)

# The last commit that changed what the image is built from: anything else is not a new release,
# and must not restart Phloem under a new tag.
commit="$(git log -1 --format=%h --abbrev=7 HEAD -- "${TAG_PATHS[@]}")"
day="$(git log -1 --format=%cd --date=format:%Y-%m-%d HEAD -- "${TAG_PATHS[@]}")"
[[ -n "${commit}" && -n "${day}" ]] || die "no commit touches ${TAG_PATHS[*]}: is this the repository?"
if [[ "${SHIP_WORKTREE:-0}" == "1" ]]; then
  TAG="${day}-${commit}-wip-$(date +%H%M%S)"
  log "SHIP_WORKTREE=1: shipping the working tree as it is, as ${TAG}"
else
  TAG="${day}-${commit}"
  dirty="$(git status --porcelain -- "${TAG_PATHS[@]}")"
  if [[ -n "${dirty}" ]]; then
    printf '%s\n' "${dirty}" >&2
    die "the changes above are not committed, and a release is a commit. Commit them, or ship the working tree as it is with SHIP_WORKTREE=1"
  fi
fi
log "Phloem release ${TAG}${INSTANCE:+ for ${INSTANCE}}"

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
# native PATH -> the path as a native Windows program has to be told it (ship.sh says why).
native() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -m "$1"
  else
    printf '%s' "$1"
  fi
}
cleanup() {
  if [[ "${SHIP_KEEP:-0}" == "1" || "${BUILD_ONLY}" == "1" ]]; then
    log "the image file stays in ${WORK}"
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
  loaded="$(remote "docker image inspect betula-phloem:${TAG} >/dev/null 2>&1 && echo yes || echo no" || true)"
  if [[ "${loaded}" == "yes" ]]; then
    log "the image of ${TAG} is on the server already: nothing to build"
    needs_build=0
  fi
fi

if [[ "${needs_build}" == "1" ]]; then
  log "exporting the sources (${EXPORT_PATHS[*]})"
  tree="HEAD"
  if [[ "${SHIP_WORKTREE:-0}" == "1" ]]; then
    # The working tree as a tree object, made with an index of its own: what git would commit
    # (line endings included), tracked or not, never ignored files. The real index, HEAD and the
    # working files are not touched.
    tree="$(
      GIT_INDEX_FILE="$(native "${WORK}/index")"
      export GIT_INDEX_FILE
      git read-tree HEAD
      git add -A -- "${EXPORT_PATHS[@]}"
      git write-tree
    )"
  fi
  # As the repository stores them, byte for byte, whatever this machine's checkout does with line
  # endings (ship.sh says what happens otherwise).
  git -c core.autocrlf=false -c core.eol=lf archive --format=tar.gz -o "$(native "${WORK}/src.tar.gz")" "${tree}" -- "${EXPORT_PATHS[@]}"

  # Runs where Nix is. The sources are unpacked into that side's own file system (a build on
  # /mnt/c would crawl), into a directory of their own (ship.sh and ship-cortex.sh may build next
  # to it), built as a "path:" flake (no git needed there), and the image file (a docker archive,
  # gzip) is copied back next to the script.
  cat >"${WORK}/build.sh" <<'BUILD'
#!/usr/bin/env bash
set -Eeuo pipefail
work="$1"
src="${XDG_CACHE_HOME:-$HOME/.cache}/betula-ship-phloem/src"
rm -rf -- "${src}"
mkdir -p "${src}"
tar -xzf "${work}/src.tar.gz" -C "${src}"
echo "[build] nix build .#phloem-image"
nix --extra-experimental-features 'nix-command flakes' build "path:${src}#phloem-image" --out-link "${src}/../result-phloem" --print-build-logs
cp -fL "${src}/../result-phloem" "${work}/phloem-image.tar.gz"
BUILD
  log "building the image with Nix"
  run_nix_side bash "$(inside "${WORK}/build.sh")" "$(inside "${WORK}")"
  [[ -s "${WORK}/phloem-image.tar.gz" ]] || die "the build left no phloem-image.tar.gz"
  log "built: phloem-image.tar.gz ($(du -h "${WORK}/phloem-image.tar.gz" | cut -f1))"
fi

if [[ "${BUILD_ONLY}" == "1" ]]; then
  log "--build-only: nothing was sent anywhere. Try the image: docker load < ${WORK}/phloem-image.tar.gz"
  exit 0
fi

# ---------------------------------------------------------------- load, sync, deploy

if [[ "${needs_build}" == "1" ]]; then
  log "loading betula-phloem:${TAG} on ${SSH_TARGET}"
  # The flake names every build "latest"; the release tag is given where it is loaded. The name
  # "latest" is never deployed (49-phloem.sh refuses it).
  remote "docker load -q && docker tag betula-phloem:latest betula-phloem:${TAG}" <"${WORK}/phloem-image.tar.gz"
fi

log "syncing deploy/ (stack file, the instance's file and the scripts of this release)"
SSH_TARGET="${SSH_TARGET}" SSH_OPTS="${SSH_OPTS:-}" bash "${DEPLOY_DIR}/sync.sh"

log "deploying ${INSTANCE}"
remote "bash /opt/betula/vps/49-phloem.sh ${INSTANCE} ${TAG}"
log "done: ${INSTANCE} runs ${TAG}"
