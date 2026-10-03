{
  description = "Betula: Radix keeps the catalog of BTU Cottbus-Senftenberg up to date and publishes SQLite snapshots over HTTP, Folia serves them as a web app";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # The Rust workspace is built in two steps: the dependencies once per Cargo.lock
    # (buildDepsOnly), then the workspace's own crates on top of them. A change to the app does
    # not compile every crate of Cargo.lock again, and CI keeps the first step in its cache
    # (.github/workflows/images.yml). A release tag, so that an update is a decision.
    crane.url = "github:ipetkov/crane/v0.24.0";
  };

  outputs = { self, nixpkgs, flake-utils, crane }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        craneLib = crane.mkLib pkgs;

        # radix/go.mod asks for Go 1.27. nixpkgs' default `go` lags behind a new release for a
        # while, so take the versioned attribute when it exists.
        buildGoModule = pkgs.buildGoModule.override { go = pkgs.go_1_27 or pkgs.go; };

        # goSource KEEP -> the Go module radix/ with only the paths KEEP says yes to. KEEP gets each
        # path relative to radix/ ("internal/oplog/oplog.go"); a directory it says no to is not
        # looked into.
        goSource = keep: pkgs.lib.cleanSourceWith {
          src = ./radix;
          filter = path: type: keep (pkgs.lib.removePrefix (toString ./radix + "/") (toString path));
        };
        # isIn PATHS REL: REL is one of PATHS or lies below one. leadsTo PATHS REL: REL is a
        # directory on the way to one of them.
        isIn = paths: rel: builtins.any (p: rel == p || pkgs.lib.hasPrefix (p + "/") rel) paths;
        leadsTo = paths: rel: builtins.any (p: pkgs.lib.hasPrefix (rel + "/") p) paths;

        # The Go module radix/: all Go code and what it embeds lives below cmd/ and internal/. A
        # change to the Rust workspace folia/, the docs or deploy/ rebuilds nothing here.
        goPaths = [ "go.mod" "go.sum" "cmd" "internal" ];
        goSrc = goSource (isIn goPaths);
        # What Cortex is built from: its own packages and the packages of the module they import
        # (never Radix's: docs/cortex/cortex.md). deploy/ship-cortex.sh reads this line for the release
        # tag, so it stays one line; a package missing here fails the build.
        cortexPaths = [ "go.mod" "go.sum" "cmd/cortex" "internal/cortex" "internal/metrics" "internal/oplog" "internal/secrets" "internal/version" ];
        # Radix fetches through Cortex's client, and imports nothing else of Cortex.
        cortexClient = [ "internal/cortex/client" ];

        # What both Go builds share. The dependencies are vendored from the whole module, not from
        # each build's own source (overrideModAttrs: buildGoModule runs "go mod vendor" over the
        # source of its goModules): one vendorHash for both, whatever each imports, and with one
        # name for both, one store path that is fetched once. So the flake's source has to hold the
        # whole module for either build (deploy/ship-cortex.sh exports it, not only cortexPaths).
        goCommon = {
          version = self.shortRev or self.dirtyShortRev or "dev";
          # Update after changing go.mod / go.sum, or after the first import of a package of a
          # dependency that nothing imported before ("go mod vendor" copies packages, not
          # modules): set to pkgs.lib.fakeHash, build, copy the hash Nix prints.
          vendorHash = "sha256-tFFT73vB3oTjpQaybpzq3I+alljd2zaXod+L7whFK7A=";
          overrideModAttrs = _: {
            name = "betula-go-modules";
            src = goSrc;
          };

          # modernc.org/sqlite is pure Go: a static binary without libc.
          env.CGO_ENABLED = "0";
          ldflags = [ "-s" "-w" ];

          # The tests are network-free and run during the build (those of subPackages).
          doCheck = true;
        };

        radix = buildGoModule (goCommon // {
          pname = "betula-radix";
          # The module without Cortex but with its client, so that a change to Cortex alone does
          # not change Radix's image.
          src = goSource (rel:
            isIn goPaths rel
            && (isIn cortexClient rel || leadsTo cortexClient rel || !(isIn [ "cmd/cortex" "internal/cortex" ] rel)));
          subPackages = [ "cmd/radix" ];
          meta.mainProgram = "radix";
        });

        radix-image = pkgs.dockerTools.buildLayeredImage {
          name = "betula-radix";
          tag = "latest";
          contents = [ radix pkgs.cacert ];
          # /data holds the working database and the exported snapshots.
          extraCommands = "mkdir -p data tmp && chmod 1777 tmp";
          config = {
            Entrypoint = [ "/bin/radix" ];
            Cmd = [ "run" ];
            Env = [
              "RADIX_DB=/data/radix.db"
              "RADIX_SNAPSHOT_DIR=/data/snapshot"
              "RADIX_ADDR=0.0.0.0:8090"
              "RADIX_LOG_FORMAT=json"
              "TZ=Europe/Berlin" # off-peak hours are local time; zoneinfo is embedded in the binary
              "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            ];
            ExposedPorts = { "8090/tcp" = { }; };
            Volumes = { "/data" = { }; };
            WorkingDir = "/data";
            Healthcheck = {
              Test = [ "CMD" "/bin/radix" "healthcheck" ];
              Interval = 60000000000; # 60 s, in nanoseconds
              Timeout = 10000000000;
              StartPeriod = 120000000000;
              StartInterval = 5000000000; # while starting: a new task counts as started after seconds, not after an interval
              Retries = 3;
            };
          };
        };

        # ---------------------------------------------------------------- Cortex (Go)

        # The cache between the application and the internet (docs/cortex/cortex.md, stacks/cortex.yml).
        # A binary of its own from the same module. Its source is only what it is built from, so
        # that a change to Radix does not change Cortex's image either.
        cortex = buildGoModule (goCommon // {
          pname = "betula-cortex";
          src = goSource (rel: isIn cortexPaths rel || leadsTo cortexPaths rel);
          subPackages = [ "cmd/cortex" ];
          meta.mainProgram = "cortex";
        });

        cortex-image = pkgs.dockerTools.buildLayeredImage {
          name = "betula-cortex";
          tag = "latest";
          # /bin/cortex, and the CA certificates of the hosts it fetches from over https.
          contents = [ cortex pkgs.cacert ];
          # /data holds the index and the blobs, /lock the leader's lock file (a volume that both
          # instances mount, stacks/cortex.yml). Both belong to the user Cortex runs as: a fresh
          # named volume mounted there takes that owner over.
          fakeRootCommands = ''
            mkdir -p data lock tmp
            chown 10002:10002 data lock
            chmod 1777 tmp
          '';
          config = {
            Entrypoint = [ "/bin/cortex" ];
            Cmd = [ "serve" ];
            User = "10002:10002";
            Env = [
              "CORTEX_ADDR=0.0.0.0:8100"
              "CORTEX_DATA=/data"
              "CORTEX_LOG_FORMAT=json"
              "TZ=Europe/Berlin"
              "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            ];
            ExposedPorts = { "8100/tcp" = { }; };
            Volumes = { "/data" = { }; };
            WorkingDir = "/data";
            # Liveness (/livez): the process serves and its index answers. Not /healthz, which
            # fails while no leader is known or the follower lags: no reason to kill a process
            # that still serves. A leader that hangs is killed after three failed checks (half a
            # minute), and the lock it held goes to the follower.
            Healthcheck = {
              Test = [ "CMD" "/bin/cortex" "healthcheck" ];
              Interval = 10000000000; # 10 s, in nanoseconds
              Timeout = 5000000000;
              StartPeriod = 30000000000;
              StartInterval = 2000000000; # a new task counts as started after seconds, not after an interval
              Retries = 3;
            };
          };
        };

        # ---------------------------------------------------------------- Folia (Rust)

        # Only the Cargo workspace folia/ (its crates and the assets the server embeds): a change
        # to the Go module, the docs or Folia's e2e checks and scripts rebuilds nothing here.
        rustSrc = pkgs.lib.cleanSourceWith {
          src = ./folia;
          filter = path: type:
            let
              rel = pkgs.lib.removePrefix (toString ./folia + "/") (toString path);
              top = builtins.head (pkgs.lib.splitString "/" rel);
            in
            builtins.elem top [ "Cargo.toml" "Cargo.lock" "crates" "assets" ];
        };

        cargoLock = builtins.fromTOML (builtins.readFile ./folia/Cargo.lock);
        lockedVersion = name:
          (pkgs.lib.findFirst (p: p.name == name) (throw "${name} is not in Cargo.lock") cargoLock.package).version;
        foliaVersion = (builtins.fromTOML (builtins.readFile ./folia/crates/server/Cargo.toml)).package.version;

        # What both Rust builds share. No hash to keep up to date: every crate is fetched by its
        # checksum in Cargo.lock.
        rustCommon = {
          src = rustSrc;
          strictDeps = true;
          # The tests need a catalog snapshot (docs/folia/frontend.md §4); they run on the workstation.
          doCheck = false;
        };

        # The web server: `cargo build --release -p folia-server`, in two derivations. The first
        # compiles the dependencies from a copy of the workspace whose own sources are dummies
        # (crane's mkDummySrc), so it only changes with Cargo.lock and the manifests; its version
        # is fixed, so that the server's version number does not rebuild it either. The second
        # starts from its target directory and compiles the workspace's crates.
        serverArgs = rustCommon // {
          pname = "betula-folia";
          cargoExtraArgs = "--locked -p folia-server";
        };
        folia-deps = craneLib.buildDepsOnly (serverArgs // {
          version = "0";
          # Only what the second step reuses: no `cargo check`, no test builds.
          buildPhaseCargoCommand = "cargoWithProfile build --locked -p folia-server";
        });
        folia = craneLib.buildPackage (serverArgs // {
          version = foliaVersion;
          cargoArtifacts = folia-deps;
          meta.mainProgram = "folia";
        });

        # The wasm-bindgen CLI has to be exactly the version of the crate the browser app is built
        # with (folia/crates/client/Cargo.toml pins it), and nixpkgs rarely has that one. After a change of the
        # version: set both hashes to pkgs.lib.fakeHash, build, copy the hash Nix prints, twice.
        wasm-bindgen-cli = pkgs.buildWasmBindgenCli rec {
          src = pkgs.fetchCrate {
            pname = "wasm-bindgen-cli";
            version = lockedVersion "wasm-bindgen";
            hash = "sha256-a7lcXJnnZkYReja+iUO7NqqrWyv3toxnUgQb8s4IS5s=";
          };
          cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
            inherit src;
            inherit (src) pname version;
            hash = "sha256-R1Tas33Ursy8kqsxguAkG0ZhNed2n5uFTAhw1l2qlLY=";
          };
        };

        # The browser app, as folia/scripts/build-client.sh builds it: site/pkg/folia_client{.js,_bg.wasm},
        # and the data worker with its own bundle (site/pkg/data-worker.js, folia_worker{.js,_bg.wasm}).
        # `cargo build --profile wasm-release --target wasm32-unknown-unknown -p folia-client -p folia-worker`, in
        # the same two steps as the web server (the dependencies apart, a fixed version for them).
        clientArgs = rustCommon // {
          pname = "betula-folia-client";
          CARGO_PROFILE = "wasm-release";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          # nixpkgs' rustc brings the wasm32 standard library, but no rust-lld to link with.
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";
          buildPhaseCargoCommand = "cargoWithProfile build --locked -p folia-client -p folia-worker";
        };
        folia-client-deps = craneLib.buildDepsOnly (clientArgs // {
          version = "0";
          nativeBuildInputs = [ pkgs.lld ];
        });
        folia-client = craneLib.mkCargoDerivation (clientArgs // {
          version = foliaVersion;
          cargoArtifacts = folia-client-deps;
          nativeBuildInputs = [
            wasm-bindgen-cli
            pkgs.lld
            # buildPackage brings these two, mkCargoDerivation does not. The panic messages in the
            # bundle name the store paths of the vendored crates and of the toolchain; without the
            # hooks the image would carry the sources of every crate in Cargo.lock.
            craneLib.removeReferencesToVendoredSourcesHook
            craneLib.removeReferencesToRustToolchainHook
          ];
          # Without the names of its functions and its producers (--remove-name-section,
          # --remove-producers-section), as folia/scripts/build-client.sh builds it: the names were 29 of
          # the bundle's 33.8 MB (docs/folia/frontend.md §3).
          installPhaseCommand = ''
            mkdir -p "$out/site/pkg"
            wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section \
              --out-dir "$out/site/pkg" --out-name folia_client \
              target/wasm32-unknown-unknown/wasm-release/folia_client.wasm
            wasm-bindgen --target web --no-typescript --remove-name-section --remove-producers-section \
              --out-dir "$out/site/pkg" --out-name folia_worker \
              target/wasm32-unknown-unknown/wasm-release/folia_worker.wasm
            cp crates/worker/js/data-worker.js "$out/site/pkg/data-worker.js"
          '';
          # The bundle is the output, not cargo's target directory.
          doInstallCargoArtifacts = false;
        });

        # The Web Worker of the semantic search, as folia/scripts/build-semantic.sh builds it into
        # site/pkg: the crate folia/crates/semantic twice (WASM SIMD, and relaxed SIMD for the browsers that
        # have it), and its two scripts. Nothing to build ahead: the crate has no dependencies.
        folia-semantic = craneLib.mkCargoDerivation (rustCommon // {
          pname = "betula-folia-semantic";
          version = foliaVersion;
          cargoArtifacts = null;
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";
          nativeBuildInputs = [ pkgs.lld ];
          buildPhaseCargoCommand = ''
            for build in simd relaxed; do
              features="+simd128"
              [ "$build" = relaxed ] && features="+simd128,+relaxed-simd"
              CARGO_ENCODED_RUSTFLAGS="-Ctarget-feature=$features" cargo rustc --locked -p folia-semantic --lib --features worker \
                --crate-type cdylib --target wasm32-unknown-unknown --profile wasm-release --target-dir "target/semantic-$build"
            done
          '';
          installPhaseCommand = ''
            mkdir -p "$out/site/pkg"
            for build in simd relaxed; do
              cp "target/semantic-$build/wasm32-unknown-unknown/wasm-release/folia_semantic.wasm" "$out/site/pkg/semantic.$build.wasm"
            done
            cp crates/semantic/js/worker.js "$out/site/pkg/semantic-worker.js"
            cp crates/semantic/js/semantic.js "$out/site/pkg/semantic.js"
          '';
          doInstallCargoArtifacts = false;
        });

        folia-image = pkgs.dockerTools.buildLayeredImage {
          name = "betula-folia";
          tag = "latest";
          # /bin/folia and /site/pkg. No CA certificates: Folia only speaks plain HTTP, to Radix.
          contents = [ folia folia-client folia-semantic ];
          # /data holds the downloaded snapshots. It belongs to the user the server runs as, and a
          # fresh named volume mounted there takes that owner over.
          fakeRootCommands = ''
            mkdir -p data tmp
            chown 10001:10001 data
            chmod 1777 tmp
          '';
          config = {
            Entrypoint = [ "/bin/folia" ];
            User = "10001:10001";
            Env = [
              "FOLIA_ADDR=0.0.0.0:8080"
              "FOLIA_DATA_DIR=/data"
              "FOLIA_SITE_ROOT=/site"
              "FOLIA_SNAPSHOT_URL=http://radix:8090/snapshot/catalog.db"
              "FOLIA_LOG_FORMAT=json"
            ];
            ExposedPorts = { "8080/tcp" = { }; };
            Volumes = { "/data" = { }; };
            WorkingDir = "/data";
            # Liveness (/livez), not /healthz: that one fails before the first snapshot and while
            # Radix is silent, which is no reason to restart a server that still serves pages.
            Healthcheck = {
              Test = [ "CMD" "/bin/folia" "healthcheck" ];
              Interval = 30000000000; # 30 s, in nanoseconds
              Timeout = 5000000000;
              StartPeriod = 30000000000;
              StartInterval = 2000000000; # a new task takes over after seconds, not after an interval
              Retries = 3;
            };
          };
        };
      in
      {
        packages = {
          inherit radix radix-image cortex cortex-image folia folia-client folia-semantic folia-image;
          default = radix;
        };

        apps.default = flake-utils.lib.mkApp { drv = radix; };

        devShells.default = pkgs.mkShell {
          packages = [ (pkgs.go_1_27 or pkgs.go) pkgs.gopls pkgs.sqlite ];
        };
      });
}
