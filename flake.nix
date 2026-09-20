{
  description = "Betula: Radix keeps the catalog of BTU Cottbus-Senftenberg up to date and publishes SQLite snapshots over HTTP";

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
            # Only the Go module: the Rust web server and the frontend are built elsewhere.
            filter = path: type:
              let rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
              in !(pkgs.lib.hasPrefix "frontend" rel || pkgs.lib.hasPrefix "server" rel
                || pkgs.lib.hasPrefix "target" rel || pkgs.lib.hasPrefix "docs" rel);
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
              Retries = 3;
            };
          };
        };
      in
      {
        packages = {
          inherit radix radix-image;
          default = radix;
        };

        apps.default = flake-utils.lib.mkApp { drv = radix; };

        devShells.default = pkgs.mkShell {
          packages = [ (pkgs.go_1_27 or pkgs.go) pkgs.gopls pkgs.sqlite ];
        };
      });
}
