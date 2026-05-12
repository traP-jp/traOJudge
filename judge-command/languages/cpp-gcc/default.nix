{
  pkgs,
  ...
}:
{
  trao.languages.cpp-gcc-23 = {
    builder =
      sourceFile:
      pkgs.stdenv.mkDerivation rec {
        name = "trao_build_cpp_gcc_23";
        src = pkgs.writeTextDir "main.cpp" (builtins.readFile sourceFile);
        buildInput = with pkgs; [ gcc ];
        buildPhase = ''
          g++ -std=c++23 main.cpp -o /build/main
        '';
        installPhase = ''
          mkdir -p $out/bin
          cp /build/main $out/bin/${name}
        '';
      };
    prebuildSourceFile = ./main.cpp;
    displayName = "C++ (GCC 23)";
    extension = "cpp";
  };
}
