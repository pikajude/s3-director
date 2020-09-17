with import <nixpkgs> { };
mkShell {
  name = "s3-director";
  APP_PORT = 10101;
  AWS_REGION = "us-west-2";
  AWS_S3_BUCKET = "dzycnjinlzdxzgciyfkp";
  buildInputs = [ pkg-config openssl ];
}
