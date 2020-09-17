use hyper::{
  service::{make_service_fn, service_fn},
  Body, Request, Response, Server, StatusCode,
};
use rusoto_core::{credential::*, region::Region, signature::SignedRequest};
use std::{convert::Infallible, env, error::Error, net::ToSocketAddrs, sync::Arc, time::Duration};

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
  // We could return a 404 here and save a few cycles, but this is what the
  // upstream aws-s3-proxy does.

  // if s3_req.key.is_empty() {
  // }

  let mut req = SignedRequest::new("GET", "s3", &state.config.region, req.uri().path());
  req.set_hostname(Some(format!("{}.s3.amazonaws.com", &state.config.bucket)));

  let uri = req.generate_presigned_url(&state.creds, &Duration::from_secs(3600), false);

  Ok(
    Response::builder()
      .status(StatusCode::FOUND)
      .header("Location", &uri)
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
