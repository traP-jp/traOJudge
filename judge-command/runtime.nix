# Trusted toolchains only. Submitted source is never interpolated into Nix.
{ ... }: {
  perSystem = { pkgs, ... }: let
    runtime = name: tools: manifest: pkgs.buildEnv {
      name = "judge-runtime-${name}";
      paths = tools ++ [ pkgs.bash pkgs.coreutils (pkgs.writeTextDir
        "share/traojudge/language.json" (builtins.toJSON (manifest // {
          schema = 1;
          language = name;
        }))) ];
    };
  in {
    packages = (pkgs.lib.optionalAttrs pkgs.stdenv.isLinux {
      probe-tools = pkgs.buildEnv {
        name = "judge-probe-tools";
        paths = [ pkgs.docker-client (pkgs.python3.withPackages (p: [p.boto3]))
          (pkgs.writeShellScriptBin "probe" ''
            exec ${pkgs.stdenv.cc.bintools.dynamicLinker} \
              --library-path ${pkgs.lib.makeLibraryPath [pkgs.glibc pkgs.stdenv.cc.cc.lib]} \
              /opt/judge-probe "$@"
          '') ];
      };
    }) // {
      runtime-cpp = runtime "cpp" [ pkgs.gcc ] {
        source = "main.cpp";
        compile = [ "${pkgs.gcc}/bin/g++" "-std=c++23" "-O2" "-pipe" "main.cpp" "-o" "program" ];
        run = [ "./program" ];
        version = pkgs.gcc.version;
      };
      runtime-python = runtime "python" [ pkgs.python3 ] {
        source = "main.py";
        compile = [];
        run = [ "${pkgs.python3}/bin/python3" "-I" "-B" "main.py" ];
        version = pkgs.python3.version;
      };
      runtime-rust = runtime "rust" [ pkgs.rustc pkgs.gcc ] {
        source = "main.rs";
        compile = [ "${pkgs.rustc}/bin/rustc" "--edition=2024" "-O" "-C" "codegen-units=1" "main.rs" "-o" "program" ];
        run = [ "./program" ];
        version = pkgs.rustc.version;
      };
    };
  };
}
