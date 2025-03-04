{ lib
, rustPlatform
, openssl
, pkg-config
}:
let
  fs = lib.fileset;
  sourceFiles = fs.unions [
    ./Cargo.toml
    ./Cargo.lock
    ./src/main.rs
  ];
in
rustPlatform.buildRustPackage {
  pname = "cheap-ichnaea";
  version = "0.3.0";

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

  meta = with lib; {
    description = "Mimics the ichnaea geolocate v1 API";
    license = licenses.mit;
  };
}
