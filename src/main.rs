use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Client, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use hyper::http::uri;
use hyper::http::request;

type Error = Box<dyn std::error::Error + Send + Sync>;


async fn forward_request(req: Request<Body>) -> Result<Response<Body>, Error> {
    println!("Request: {:?}", req);
    let (mut parts, body) = req.into_parts();
    let path_and_query = match parts.uri.path_and_query(){
        Some(s) => s.clone(),
        None => "/".parse()?
    };
    parts.uri = hyper::Uri::builder()
        .scheme("http")
        .authority("localhost:8000")
        .path_and_query(path_and_query)
        .build()?;

    let client = Client::new();
    let resp = client.request(Request::from_parts(parts, body)).await?;
    println!("Resp: {:?}", resp);
    Ok(resp)
}


#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    let svc = make_service_fn(|_conn| async {
        Ok::<_, Error>(service_fn(forward_request))
    });

    let server = Server::bind(&addr).serve(svc);

    if let Err(e) = server.await {
        eprintln!("Server error {}", e);
    }
}
