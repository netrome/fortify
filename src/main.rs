use std::net::SocketAddr;
use hyper::{Body, Client, Method, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};

type Error = Box<dyn std::error::Error + Send + Sync>;

static LOGIN_FORM: &'static str = include_str!("login_form.html");


fn replace_authority(replacement: &str, req: Request<Body>) -> Result<Request<Body>, Error>{
    let (mut parts, body) = req.into_parts();
    let path_and_query = match parts.uri.path_and_query(){
        Some(s) => s.clone(),
        None => "/".parse()?
    };
    parts.uri = hyper::Uri::builder()
        .scheme("http")
        .authority(replacement)
        .path_and_query(path_and_query)
        .build()?;
    Ok(Request::from_parts(parts, body))
}


async fn prompt_login(_req: Request<Body>) -> Result<Response<Body>, Error> {
    match Response::builder()
        .status(200)
        .body(Body::from(LOGIN_FORM)) {
            Ok(resp) => Ok(resp),
            Err(error) => Err(Box::new(error))
        }
}


async fn handle_login(_req: Request<Body>) -> Result<Response<Body>, Error> {
    match Response::builder()
        .status(200)
        .header("set-cookie", "session=1337")
        .body(Body::empty()) {
            Ok(resp) => Ok(resp),
            Err(error) => Err(Box::new(error))
        }
}


async fn forward_request(req: Request<Body>) -> Result<Response<Body>, Error> {
    let client = Client::new();
    let req = replace_authority("localhost:8000", req)?;
    let resp = client.request(req).await?;
    Ok(resp)
}


async fn route(req: Request<Body>) -> Result<Response<Body>, Error> {
    match (req.method(), req.uri().path(), req.headers().get("cookie")) {
        (&Method::POST, "/login", None) => handle_login(req).await,
        (_, _, None) => prompt_login(req).await,
        _ => forward_request(req).await
    }
}


#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let svc = make_service_fn(|_conn| async {
        Ok::<_, Error>(service_fn(route))
    });

    let server = Server::bind(&addr).serve(svc);

    if let Err(e) = server.await {
        eprintln!("Server error {}", e);
    }
}
