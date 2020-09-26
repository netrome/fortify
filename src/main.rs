use std::time::SystemTime;
use std::net::SocketAddr;

use hyper::{Body, Client, Method, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use hyper::header::HeaderValue;
use hyper::http::uri::PathAndQuery;

#[macro_use] extern crate serde_json;
use frank_jwt::{Algorithm, encode, decode};

use cookie::Cookie;

type Error = Box<dyn std::error::Error + Send + Sync>;

// ----------------------------------------------------------------

static LOGIN_FORM: &'static str = include_str!("login_form.html");


// Session management ------------------------------------------------------

fn one_hour_from_now() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("This must work")
        .as_secs()
        + 3600
}

fn session_jwt(username: &str) -> String {
    let payload = json!({
        "name": username,
        "exp": one_hour_from_now(),
    });

    let header = json!({});
    let secret = String::from("super_secret");
    encode(header, &secret, &payload, Algorithm::HS256).expect("This can't fail right?")
}

fn session_cookie(username: &str) -> String {
    Cookie::build("session", session_jwt(username))
        .http_only(true)
        .same_site(cookie::SameSite::Strict)
        .finish()
        .to_string()
}

fn decoded_cookie_username(encoded_token: &str) -> String{
    let secret = String::from("super_secret");
    let (_, payload) = decode(encoded_token, &secret, Algorithm::HS256, &frank_jwt::ValidationOptions::default()).expect("I will fix this later...");
    match &payload["name"] {
        serde_json::Value::String(content) => String::from(content),
        _ => String::from("")
    }
}

fn decoded_user_info(cookie_str: &str) -> Result<String, cookie::ParseError> {
    let cookie = Cookie::parse(cookie_str)?;
    match cookie.name() {
        "session" => Ok(format!("name={}", decoded_cookie_username(cookie.value()))),
        _ => Err(cookie::ParseError::MissingPair)
    }
}

// Handling requests --------------------------------------------


fn user_info_as_query_params(maybe_cookie: Option<&HeaderValue>) -> Result<String, cookie::ParseError> {
    match maybe_cookie {
        Some(value) => decoded_user_info(value.to_str().expect("How can this go wrong?")),
        None => Err(cookie::ParseError::EmptyName)
    }
}

fn with_extras(path_and_query: PathAndQuery, extras: &str) -> PathAndQuery{
    let pq_str = path_and_query.to_string();
    if pq_str.contains("?") {
        format!("{}&{}", pq_str, extras).parse().expect("This should never fail")
    } else {
        format!("{}?{}", pq_str, extras).parse().expect("This should also never fail")
    }
}


fn replace_authority_and_add_extras(replacement: &str, extra_parameters: &str, req: Request<Body>) -> Result<Request<Body>, Error> {
    let (mut parts, body) = req.into_parts();
    let path_and_query = match parts.uri.path_and_query(){
        Some(s) => s.clone(),
        None => "/".parse()?
    };
    let path_and_query = with_extras(path_and_query, extra_parameters);
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
        .header("set-cookie", session_cookie("Marten"))
        .body(Body::empty()) {
            Ok(resp) => Ok(resp),
            Err(error) => Err(Box::new(error))
        }
}


async fn forward_request(req: Request<Body>, extra_parameters: &str) -> Result<Response<Body>, Error> {
    let client = Client::new();
    let req = replace_authority_and_add_extras("localhost:8000", extra_parameters, req)?;
    let resp = client.request(req).await?;
    Ok(resp)
}


async fn route(req: Request<Body>) -> Result<Response<Body>, Error> {
    let extra_query_params = user_info_as_query_params(req.headers().get("cookie"));
    match (req.method(), req.uri().path(), extra_query_params) {
        (&Method::POST, "/login", Err(_)) => handle_login(req).await,
        (_, _, Err(_)) => prompt_login(req).await,
        (_, _, Ok(extra_params)) => forward_request(req, &extra_params).await
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
