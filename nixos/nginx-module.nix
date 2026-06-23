# NixOS module for deploying 保卫萝卜 with Nginx.
#
# Usage in your NixOS configuration:
#
#   {
#     imports = [ inputs.protect-carrot.nixosModules.default ];
#
#     services.protect-carrot = {
#       enable = true;
#       hostname = "game.example.com";
#       # Optional: enable HTTPS with ACME
#       enableACME = true;
#       # Optional: extra Nginx server directives
#       extraConfig = ''
#         add_header X-Frame-Options "SAMEORIGIN" always;
#       '';
#     };
#   }
#
# This module:
#   - Configures Nginx with brotli/gzip compression for wasm/js
#   - Sets correct MIME types for .wasm files
#   - Adds proper cache headers for static assets
#   - Optionally enables HTTPS via ACME/Let's Encrypt
{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.protect-carrot;
  package = cfg.package;

  # nginx's `add_header` REPLACES all parent-level headers in a location that
  # declares its own `add_header` — so every location below that sets a header
  # must re-state these security headers, or they silently vanish there. (The
  # gixy linter, which srvos runs at build time, also fails the build on this.)
  securityHeaders = ''
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header Cross-Origin-Embedder-Policy "require-corp" always;
    add_header Cross-Origin-Opener-Policy "same-origin" always;
  '';
in {
  options.services.protect-carrot = {
    enable = lib.mkEnableOption "保卫萝卜 (Protect the Carrot) web game";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.protect-carrot-web or (pkgs.callPackage ../packages/web.nix {});
      defaultText = lib.literalExpression "pkgs.protect-carrot-web";
      description = "The protect-carrot-web package to serve.";
    };

    hostname = lib.mkOption {
      type = lib.types.str;
      default = "localhost";
      description = "Hostname for the Nginx virtual host.";
    };

    listenPort = lib.mkOption {
      type = lib.types.port;
      default = 80;
      description = "Port for the Nginx virtual host to listen on.";
    };

    enableACME = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enable HTTPS via ACME (Let's Encrypt). Requires a valid domain.";
    };

    forceSSL = lib.mkOption {
      type = lib.types.bool;
      default = cfg.enableACME;
      description = "Force HTTPS redirect. Automatically enabled when enableACME is true.";
    };

    extraConfig = lib.mkOption {
      type = lib.types.lines;
      default = "";
      description = "Extra Nginx server block directives.";
    };
  };

  config = lib.mkIf cfg.enable {
    # Ensure Nginx is enabled.
    services.nginx.enable = lib.mkDefault true;

    # Add the custom MIME type for .wasm files.
    services.nginx.appendHttpConfig = ''
      types {
        application/wasm wasm;
      }
    '';

    # Virtual host configuration.
    services.nginx.virtualHosts."${cfg.hostname}" = {
      root = "${package}/share/protect-carrot";

      # Listen ports: always HTTP; add HTTPS when ACME is enabled.
      # `addr` is required by newer nixpkgs (the listen submodule has no
      # default for it), so set it explicitly on every entry.
      listen = lib.mkMerge [
        # HTTP
        [
          {
            addr = "0.0.0.0";
            port = cfg.listenPort;
            ssl = false;
          }
          {
            addr = "[::]";
            port = cfg.listenPort;
            ssl = false;
          }
        ]
        # HTTPS (only when ACME enabled)
        (lib.mkIf cfg.enableACME [
          {
            addr = "0.0.0.0";
            port = 443;
            ssl = true;
          }
          {
            addr = "[::]";
            port = 443;
            ssl = true;
          }
        ])
      ];

      # Enable brotli and gzip compression.
      extraConfig = ''
        # --- Compression ---
        brotli on;
        brotli_types
          application/javascript
          application/json
          application/wasm
          text/css
          text/html
          text/plain
          text/xml;

        gzip on;
        gzip_types
          application/javascript
          application/json
          application/wasm
          text/css
          text/html
          text/plain
          text/xml;
        gzip_vary on;

        # --- Security headers ---
        # Applied at server level for locations that add no headers of their
        # own; locations below that DO add headers must repeat these (nginx
        # add_header is replace-all, not additive).
        ${securityHeaders}

        # --- Caching ---
        # Cache immutable assets aggressively (filenames include content hash via wasm-bindgen).
        location ~* \.(wasm|js)$ {
          ${securityHeaders}
          add_header Cache-Control "public, max-age=31536000, immutable";
          # CORS headers for SharedArrayBuffer if needed.
          add_header Access-Control-Allow-Origin "*";
        }

        # Cache compressed variants.
        location ~* \.(wasm|js)\.(br|gz)$ {
          ${securityHeaders}
          add_header Cache-Control "public, max-age=31536000, immutable";
          add_header Content-Type "application/wasm";
          add_header Access-Control-Allow-Origin "*";
        }

        # Assets can also be cached long-term.
        location /assets/ {
          ${securityHeaders}
          add_header Cache-Control "public, max-age=86400";
        }

        # index.html should NOT be cached (users need the latest loader).
        location = /index.html {
          ${securityHeaders}
          add_header Cache-Control "no-cache, must-revalidate";
        }

        # wasm_size.txt is fetched by the loader — short cache.
        location = /wasm_size.txt {
          ${securityHeaders}
          add_header Cache-Control "no-cache";
        }

        # Fallback: serve index.html for SPA-style routing.
        location / {
          try_files $uri $uri/ /index.html;
        }

        ${cfg.extraConfig}
      '';

      # ACME / SSL configuration.
      enableACME = cfg.enableACME;
      forceSSL = cfg.forceSSL;
    };
  };
}
