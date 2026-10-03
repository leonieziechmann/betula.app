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

        radix = buildGoModule {
          pname = "betula-radix";
          version = self.shortRev or self.dirtyShortRev or "dev";
          # Only the Go module radix/ (all Go code and what it embeds): a change to the Rust
          # workspace folia/, the docs or deploy/ rebuilds nothing here.
          src = pkgs.lib.cleanSourceWith {
            src = ./radix;
            filter = path: type:
              let
                rel = pkgs.lib.removePrefix (toString ./radix + "/") (toString path);
                top = builtins.head (pkgs.lib.splitString "/" rel);
              in
              builtins.elem top [ "go.mod" "go.sum" "cmd" "internal" ];
          };
          subPackages = [ "cmd/radix" ];

          # Update after changing go.mod / go.sum: set to pkgs.lib.fakeHash, build, copy the hash Nix prints.
          vendorHash = "sha256-tFFT73vB3oTjpQaybpzq3I+alljd2zaXod+L7whFK7A=";

          # modernc.org/sqlite is pure Go: a static binary without libc.
          env.CGO_ENABLED = "0";
          ldflags = [ "-s" "-w" ];

          # The tests are network-free and run during the build.
          doCheck = true;

          meta.mainProgram = "radix";
        };

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
        # and the catalog's search worker, which runs the same bundle (site/pkg/search-worker.js).
        # `cargo build --profile wasm-release --target wasm32-unknown-unknown -p folia-client`, in
        # the same two steps as the web server (the dependencies apart, a fixed version for them).
        clientArgs = rustCommon // {
          pname = "betula-folia-client";
          CARGO_PROFILE = "wasm-release";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          # nixpkgs' rustc brings the wasm32 standard library, but no rust-lld to link with.
          CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";
          buildPhaseCargoCommand = "cargoWithProfile build --locked -p folia-client";
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
            cp crates/client/js/search-worker.js "$out/site/pkg/search-worker.js"
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
          inherit radix radix-image folia folia-client folia-semantic folia-image;
          default = radix;
        };

        apps.default = flake-utils.lib.mkApp { drv = radix; };

        devShells.default = pkgs.mkShell {
          packages = [ (pkgs.go_1_27 or pkgs.go) pkgs.gopls pkgs.sqlite ];
        };
      });
}
