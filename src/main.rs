use hyper::{Request, Response, Body, Server};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::sync::Arc;
use url::Url;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;

// set up the static visit count variable
static VISIT_COUNT: LazyLock<Arc<AtomicU64>> = LazyLock::new(|| Arc::new(AtomicU64::new(0)));

#[tokio::main]
async fn main() {
    // create service factory
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(handle_request))
    });

    // set up listening address
    let addr = ([127, 0, 0, 1], 8080).into();
    let server = Server::bind(&addr).serve(make_svc);
    println!("The server is currently listening on localhost:8080.");

    // accepting and handling the requests
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let uri = &req.uri().to_string()[..];
    let path = req.uri().path();

    match path {
        "/count" => {
            let count_clone = Arc::clone(&VISIT_COUNT);
            println!("Visit count: {}",count_clone.fetch_add(1, Ordering::SeqCst)+1);
        },
        "/songs/new" => {
            println!("new");
        },
        "/songs/search" => {
            println!("search");
        },
        s if s.contains("/songs/play") => {
            println!("play");
        },
        _  => {
            println!("Welcome to the Rust-powered web server!");
        },
    }

    // 你可以在这里执行更多异步任务（例如异步读取请求体）
    // 例如： let body_bytes = hyper::body::to_bytes(req.into_body()).await?;

    Ok(Response::new(Body::from("")))
}
