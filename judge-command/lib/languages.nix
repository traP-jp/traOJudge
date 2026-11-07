{ flake-parts-lib, ... }:
{
  options.perSystem = flake-parts-lib.mkPerSystemOption (
    {
      pkgs,
      lib,
      config,
      ...
    }:
    let
      cfg = config.trao.languages;
    in
    with lib;
    {
      options.trao.languages = mkOption {
        type = types.lazyAttrsOf (
          types.submodule (
            { config, ... }:
            {
              options = {
                builder = mkOption {
                  type = types.functionTo types.package;
                  description = "Represents compile-phase. { source: string } -> package";
                };
                prebuildSourceFile = mkOption {
                  type = types.path;
                  description = "A sample source code file for prebuilding.";
                };
                displayName = mkOption {
                  type = types.str;
                  description = "A human-readable name for the language.";
                };
                extension = mkOption {
                  type = types.str;
                  description = "The file extension for source files in this language.";
                };
                checks = mkOption {
                  type = types.lazyAttrsOf (
                    types.submodule (
                      { ... }:
                      {
                        options = {
                          sourceFile = mkOption {
                            type = types.path;
                            description = "The source code to run the check against.";
                          };
                          input = mkOption {
                            type = types.str;
                            description = "The input to provide to the program.";
                          };
                          expectedOutput = mkOption {
                            type = types.str;
                            description = "The expected output from the program.";
                          };
                        };
                      }
                    )
                  );
                  description = "A set of checks to validate the builder.";
                };
              };
            }
          )
        );
        default = { };
      };

      config = {
        checks = lib.foldlAttrs (
          acc1: langName: langConfig:
          lib.mkMerge [
            (lib.foldlAttrs (
              acc2: checkName: checkConfig:
              lib.mkMerge [
                (
                  let
                    builtProgram = langConfig.builder checkConfig.sourceFile;
                    builtProgramName = builtProgram.name;
                    inputFile = pkgs.writeText "input_${langName}-${checkName}" checkConfig.input;
                    expectedOutputFile = pkgs.writeText "expected-output_${langName}-${checkName}" checkConfig.expectedOutput;
                    checkedProgram = pkgs.stdenvNoCC.mkDerivation {
                      name = "run-check_${langName}-${checkName}";
                      dontUnpack = true;
                      buildPhase = ''
                        runHook preBuild
                        ${builtProgram}/bin/${builtProgramName} < ${inputFile} > output.txt
                        diff -w ${expectedOutputFile} output.txt
                        runHook postBuild
                      '';
                      installPhase = ''
                        runHook preInstall
                        touch $out
                        runHook postInstall
                      '';
                    };
                  in
                  {
                    "compile-check_${langName}-${checkName}" = builtProgram;
                    "run-check_${langName}-${checkName}" = checkedProgram;
                  }
                )
                acc2
              ]
            ) { } langConfig.checks)
            acc1
          ]
        ) { } cfg;
        packages =
          let
            eachPrebuilds = lib.mapAttrs' (
              langName: langConfig:
              lib.nameValuePair "prebuild_${langName}" (langConfig.builder langConfig.prebuildSourceFile)
            ) cfg;
          in
          eachPrebuilds
          // {
            "prebuild-all" = pkgs.symlinkJoin {
              name = "prebuild-all";
              paths = lib.attrValues eachPrebuilds;
            };
          };
      };
    }
  );
}
