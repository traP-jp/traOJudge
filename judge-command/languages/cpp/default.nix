{
  pkgs,
  ...
}:
{
  trao.languages = {
    cpp-23-gcc = {
      builder =
        sourceFile:
        pkgs.stdenv.mkDerivation rec {
          name = "trao_build_cpp_23_gcc";
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
      checks = {
        helloWorld = {
          sourceFile = ./main.cpp;
          input = "";
          expectedOutput = "Hello, World!";
        };
      };
    };
    cpp-23-clang = {
      builder =
        sourceFile:
        pkgs.stdenv.mkDerivation rec {
          name = "trao_build_cpp_23_clang";
          src = pkgs.writeTextDir "main.cpp" (builtins.readFile sourceFile);
          buildInputs = with pkgs; [ clang ];
          buildPhase = ''
            clang++ -std=c++23 main.cpp -o /build/main
          '';
          installPhase = ''
            mkdir -p $out/bin
            cp /build/main $out/bin/${name}
          '';
        };
      prebuildSourceFile = ./main.cpp;
      displayName = "C++ (GCC 23)";
      extension = "cpp";
      checks = {
        helloWorld = {
          sourceFile = ./main.cpp;
          input = "";
          expectedOutput = "Hello, World!";
        };
      };
    };
  };
}
