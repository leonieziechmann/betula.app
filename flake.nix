{
  description = "Betula: Radix keeps the catalog of BTU Cottbus-Senftenberg up to date and publishes SQLite snapshots over HTTP, Folia serves them as a web app";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # go.mod asks for Go 1.27. nixpkgs' default `go` lags behind a new release for a
        # while, so take the versioned attribute when it exists.
        buildGoModule = pkgs.buildGoModule.override { go = pkgs.go_1_27 or pkgs.go; };

        radix = buildGoModule {
          pname = "betula-radix";
          version = self.shortRev or self.dirtyShortRev or "dev";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            # Only the Go module (all Go code and what it embeds lives below cmd/ and internal/):
            # a change to the Rust workspace, the docs or deploy/ rebuilds nothing here.
            filter = path: type:
              let
                rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
                top = builtins.head (pkgs.lib.splitString "/" rel);
              in
              builtins.elem top [ "go.mod" "go.sum" "cmd" "internal" ];
          };
          subPackages = [ "cmd/radix" ];

          # Update after changing go.mod / go.sum: set to pkgs.lib.fakeHash, build, copy the hash Nix prints.
          vendorHash = "sha256-b33lF4UjPtoTE0qbJ8mOmjEdxsLwUJqv3d7GjluATiA=";

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

        # Only the Cargo workspace: a change to the Go module or the docs rebuilds nothing here.
        rustSrc = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let
              rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
              top = builtins.head (pkgs.lib.splitString "/" rel);
            in
            builtins.elem top [ "Cargo.toml" "Cargo.lock" "app" "catalog" "client" "pack" "server" ];
        };

        cargoLock = builtins.fromTOML (builtins.readFile ./Cargo.lock);
        lockedVersion = name:
          (pkgs.lib.findFirst (p: p.name == name) (throw "${name} is not in Cargo.lock") cargoLock.package).version;
        foliaVersion = (builtins.fromTOML (builtins.readFile ./server/Cargo.toml)).package.version;

        folia = pkgs.rustPlatform.buildRustPackage {
          pname = "betula-folia";
          version = foliaVersion;
          src = rustSrc;
          # No hash to keep up to date: every crate is fetched by its checksum in Cargo.lock.
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "folia-server" ];

          # The tests need a catalog snapshot (docs/frontend.md §4); they run on the workstation.
          doCheck = false;

          meta.mainProgram = "folia";
        };

        # The wasm-bindgen CLI has to be exactly the version of the crate the browser app is built
        # with (client/Cargo.toml pins it), and nixpkgs rarely has that one. After a change of the
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

        # The browser app, as scripts/build-client.sh builds it: site/pkg/folia_client{.js,_bg.wasm}.
        folia-client = pkgs.rustPlatform.buildRustPackage {
          pname = "betula-folia-client";
          version = foliaVersion;
          src = rustSrc;
          cargoLock.lockFile = ./Cargo.lock;

          # nixpkgs' rustc brings the wasm32 standard library, but no rust-lld to link with.
          nativeBuildInputs = [ wasm-bindgen-cli pkgs.lld ];
          env.CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER = "lld";

          buildPhase = ''
            runHook preBuild
            cargo build -p folia-client --target wasm32-unknown-unknown --profile wasm-release --offline -j "$NIX_BUILD_CORES"
            runHook postBuild
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/site/pkg"
            wasm-bindgen --target web --no-typescript --out-dir "$out/site/pkg" --out-name folia_client \
              target/wasm32-unknown-unknown/wasm-release/folia_client.wasm
            runHook postInstall
          '';
          doCheck = false;
        };

        folia-image = pkgs.dockerTools.buildLayeredImage {
          name = "betula-folia";
          tag = "latest";
          # /bin/folia and /site/pkg. No CA certificates: Folia only speaks plain HTTP, to Radix.
          contents = [ folia folia-client ];
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
          inherit radix radix-image folia folia-client folia-image;
          default = radix;
        };

        apps.default = flake-utils.lib.mkApp { drv = radix; };

        devShells.default = pkgs.mkShell {
          packages = [ (pkgs.go_1_27 or pkgs.go) pkgs.gopls pkgs.sqlite ];
        };
      });
}
