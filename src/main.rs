use hyper::{
  service::{make_service_fn, service_fn},
  Body, Request, Response, Server, StatusCode,
};
use rusoto_core::{credential::*, region::Region};
use rusoto_s3::{
  util::{PreSignedRequest, PreSignedRequestOption},
  GetObjectRequest,
};
use std::{convert::Infallible, env, error::Error, net::ToSocketAddrs, sync::Arc, time::Duration};

struct Config {
  port: u16,
  host: String,
  region: Region,
  bucket: String,
}

struct RequestState {
  config: Config,
  creds: DefaultCredentialsProvider,
}

async fn serve(state: Arc<RequestState>, req: Request<Body>) -> Result<Response<Body>, Infallible> {
  // Need to check on every req if we should refresh or not. rusoto does not stop
  // us from generating signed URIs with expired tokens (and it probably shouldn't
  // tbh)
  match state.creds.credentials().await {
    Ok(c) => {
      let mut get_obj = GetObjectRequest::default();
      get_obj.bucket = state.config.bucket.clone();
      get_obj.key = req
        .uri()
        .path()
        .strip_prefix("/")
        .expect("URI paths always start with a slash")
        .to_string();
      let signed = get_obj.get_presigned_url(
        &state.config.region,
        &c,
        &PreSignedRequestOption {
          expires_in: Duration::from_secs(24 * 60 * 60),
        },
      );

      Ok(
        Response::builder()
          .status(StatusCode::FOUND)
          .header("Location", &signed)
          .body(Body::empty())
          .unwrap(),
      )
    }
    Err(e) => Ok(
      Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from(format!(
          "Internal error (auth-related) while generating a cache URI: {}\nPlease notify \
           #eng-infra on Slack.",
          e
        )))
        .unwrap(),
    ),
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let cfg = Config {
    port: env::var("APP_PORT")?.parse()?,
    host: env::var("APP_HOST").unwrap_or_else(|_| "localhost".into()),
    region: env::var("AWS_REGION")?.parse()?,
    bucket: env::var("AWS_S3_BUCKET")?,
  };

  let mut addr = format!("{}:{}", &cfg.host, cfg.port).to_socket_addrs()?;

  let state = Arc::new(RequestState {
    creds: DefaultCredentialsProvider::new()?,
    config: cfg,
  });

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
