use hyper::{
  service::{make_service_fn, service_fn},
  Body, Request, Response, Server, StatusCode,
};
use rusoto_core::{credential::*, region::Region};
use rusoto_s3::{util::PreSignedRequest, GetObjectRequest};
use std::{convert::Infallible, env, error::Error, net::ToSocketAddrs, sync::Arc};

struct Config {
  port: u16,
  host: String,
  region: Region,
  bucket: String,
}

struct RequestState {
  config: Config,
  creds: AwsCredentials,
}

async fn serve(state: Arc<RequestState>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
  let mut s3_req = GetObjectRequest::default();
  s3_req.bucket = state.config.bucket.clone();
  s3_req.key = req
    .uri()
    .path()
    .strip_prefix("/")
    .expect("URI path always starts with a slash")
    .to_string();

  // We could return a 404 here and save a few cycles, but this is what the
  // upstream aws-s3-proxy does.

  // if s3_req.key.is_empty() {
  // }

  let redirect_to =
    s3_req.get_presigned_url(&state.config.region, &state.creds, &Default::default());

  Ok(
    Response::builder()
      .status(StatusCode::FOUND)
      .header("Location", redirect_to)
      .body(Body::empty())
      .unwrap(),
  )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let cfg = Config {
    port: env::var("APP_PORT")?.parse()?,
    host: env::var("APP_HOST").unwrap_or_else(|_| "localhost".into()),
    region: env::var("AWS_REGION")?.parse()?,
    bucket: env::var("AWS_S3_BUCKET")?,
  };

  let prof = DefaultCredentialsProvider::new()?;
  let creds = prof.credentials().await?;

  let mut addr = format!("{}:{}", &cfg.host, cfg.port).to_socket_addrs()?;

  let state = Arc::new(RequestState { creds, config: cfg });

  Ok(
    Server::bind(&addr.next().expect("host did not resolve"))
      .serve(make_service_fn(|_| {
        // fix this accursed double clone somehow, someday
        let state0 = Arc::clone(&state);
        async { Ok::<_, Infallible>(service_fn(move |r| serve(Arc::clone(&state0), r))) }
      }))
      .await?,
  )
}
