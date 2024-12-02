use hyper::{Request, Response, Body, Server};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};
use std::sync::LazyLock;
use std::fs::read_to_string;
use serde::{Deserialize};
use serde_json;

#[derive(Deserialize, Debug)]
struct Song {
    id: usize,
    title: String,
    artist: String,
    genre: String,
    play_count: usize
}

impl Display for Song {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{{\"id\":{},\"title\":\"{}\",\"artist\":\"{}\",\"genre\":\"{}\",\"play_count\":{}}}",
            self.id, self.title, self.artist, self.genre, self.play_count
        )
    }
}

#[derive(Deserialize, Debug)]
struct RawSong {
    title: String,
    artist: String,
    genre: String,
}

// site-wide visit count
static VISIT_COUNT: LazyLock<Arc<Mutex<usize>>> = LazyLock::new(|| Arc::new(Mutex::new(0)));
// the data structure that stores a musical library
static MUSICAL_LIBRARY: LazyLock<Arc<Mutex<Vec<Song>>>> = LazyLock::new(|| {
    let read_result = read_to_string("MUSICAL_LIBRARY.txt");
    match read_result {
        Ok(content) => {
            let parsed: Result<Vec<Song>, serde_json::Error> = serde_json::from_str(&content);
            match parsed {
                Ok(songs) => {
                    Arc::new(Mutex::new(songs))
                }
                Err(e) => {
                    println!("Error parsing JSON: {}", e);
                    Arc::new(Mutex::new(vec![]))
                }
            }
        }
        Err(e) => {
            println!("Error reading the file: {}", e);
            Arc::new(Mutex::new(vec![]))
        }
    }
});

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
    let path = req.uri().path();

    match path {
        "/count" => {
            let count_clone = Arc::clone(&VISIT_COUNT);
            let mut num = count_clone.lock().unwrap();
            *num += 1;
            println!("Visit count: {}",*num);
        },
        "/songs/new" => {
            // read the request body asynchronously
            match hyper::body::to_bytes(req.into_body()).await {
                Ok(body_bytes) => {
                    // parse the string into a "Song" instance
                    let body_str = String::from_utf8_lossy(&body_bytes);
                    let parsed: Result<RawSong, serde_json::Error> = serde_json::from_str(&body_str);
                    match parsed {
                        Ok(raw_song) => {
                            let library_clone = Arc::clone(&MUSICAL_LIBRARY);
                            let mut library = library_clone.lock().unwrap();
                            let song = Song {
                                id: (*library).len() + 1,
                                title: raw_song.title,
                                artist: raw_song.artist,
                                genre: raw_song.genre,
                                play_count: 0
                            };
                            println!("{}",song);
                            (*library).push(song);
                        }
                        Err(e) => {
                            println!("Error parsing JSON: {}", e);
                        }
                    }
                },
                Err(e) => {
                    println!("Error reading request body: {}", e);
                }
            }
        },
        "/songs/search" => {
            let uri = &req.uri().to_string()[..];
            println!("search");
        },
        s if s.starts_with("/songs/play") => {
            let uri = &req.uri().to_string()[..];
            println!("play");
        },
        _  => {
            println!("Welcome to the Rust-powered web server!");
        },
    }
    Ok(Response::new(Body::from("")))
}