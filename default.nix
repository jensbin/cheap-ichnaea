{ lib
, rustPlatform
, openssl
, pkg-config
}:
let
  fs = lib.fileset;
  # sourceFiles = ./.;
  # sourceFiles = fs.gitTracked ./.;
  sourceFiles = fs.unions [
    ./Cargo.toml
    ./Cargo.lock
    ./src/main.rs
    # (fs.fileFilter
    #   (file: file.hasExt "rs")
    #   ./src
    # )
  ];
in
rustPlatform.buildRustPackage {
  pname = "cheap-ichnaea";
  version = "0.2.1";

  src = fs.toSource {
    root = ./.;
    fileset = sourceFiles;
  };

  # cargoHash = lib.fakeHash;
  cargoLock = {                                                                                                                                                     
    lockFile = ./Cargo.lock;                                                                                                                                        
  };                       

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
  ];

  installPhase = ''
    runHook preInstall

    find .
    install -D -t $out/bin target/x86_64-unknown-linux-gnu/release/cheap-ichnaea

    runHook postInstall
  '';
  #doCheck = false;

  meta = with lib; {
    description = "Cheap ichnaea service";
    license = licenses.mit;
  };
}
