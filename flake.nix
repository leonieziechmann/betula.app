{
  description = "BTU catalog scraper: keeps the catalog up to date and publishes SQLite snapshots over HTTP";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        scraper = pkgs.buildGoModule {
          pname = "btu-scraper";
          version = self.shortRev or self.dirtyShortRev or "dev";
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            # Only the Go module: the Rust web server and the frontend are built elsewhere.
            filter = path: type:
              let rel = pkgs.lib.removePrefix (toString ./. + "/") (toString path);
              in !(pkgs.lib.hasPrefix "frontend" rel || pkgs.lib.hasPrefix "server" rel
                || pkgs.lib.hasPrefix "target" rel || pkgs.lib.hasPrefix "docs" rel);
          };
          subPackages = [ "cmd/scraper" ];

          # Update after changing go.mod / go.sum: set to pkgs.lib.fakeHash, build, copy the hash Nix prints.
          vendorHash = pkgs.lib.fakeHash;

          # modernc.org/sqlite is pure Go: a static binary without libc.
          env.CGO_ENABLED = "0";
          ldflags = [ "-s" "-w" ];

          # The tests are network-free and run during the build.
          doCheck = true;

          meta.mainProgram = "scraper";
        };

        container = pkgs.dockerTools.buildLayeredImage {
          name = "btu-scraper";
          tag = "latest";
          contents = [ scraper pkgs.cacert ];
          # /data holds the working database and the exported snapshots.
          extraCommands = "mkdir -p data tmp && chmod 1777 tmp";
          config = {
            Entrypoint = [ "/bin/scraper" ];
            Cmd = [ "run" ];
            Env = [
              "BTU_DB=/data/btu_v2.db"
              "BTU_SNAPSHOT_DIR=/data/snapshot"
              "BTU_ADDR=0.0.0.0:8090"
              "BTU_LOG_FORMAT=json"
              "TZ=Europe/Berlin" # off-peak hours are local time; zoneinfo is embedded in the binary
              "SSL_CERT_FILE=${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt"
            ];
            ExposedPorts = { "8090/tcp" = { }; };
            Volumes = { "/data" = { }; };
            WorkingDir = "/data";
            Healthcheck = {
              Test = [ "CMD" "/bin/scraper" "healthcheck" ];
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
          inherit scraper container;
          default = scraper;
        };

        apps.default = flake-utils.lib.mkApp { drv = scraper; };

        devShells.default = pkgs.mkShell {
          packages = [ pkgs.go pkgs.gopls pkgs.sqlite ];
        };
      });
}
