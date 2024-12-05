use hyper::{Request, Response, Body, Server};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::sync::{Arc, Mutex, LazyLock};
use tokio::sync::RwLock;
use std::fs::{read_to_string, OpenOptions};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio::signal;
use std::io::Write;

#[derive(Serialize, Deserialize, Clone)]
struct Song {
    id: usize,
    title: String,
    artist: String,
    genre: String,
    play_count: usize
}

#[derive(Deserialize)]
struct RawSong {
    title: String,
    artist: String,
    genre: String,
}

// site-wide visit count
static VISIT_COUNT: LazyLock<Arc<Mutex<usize>>> = LazyLock::new(|| Arc::new(Mutex::new(0)));
// the data structure that stores a musical library
static MUSICAL_LIBRARY: LazyLock<Arc<RwLock<Vec<Song>>>> = LazyLock::new(|| {
    let read_result = read_to_string("MUSICAL_LIBRARY.txt");
    match read_result {
        Ok(content) => {
            let parsed: Result<Vec<Song>, serde_json::Error> = serde_json::from_str(&content);
            match parsed {
                Ok(songs) => {
                    Arc::new(RwLock::new(songs))
                }
                Err(e) => {
                    println!("Error parsing JSON: {}", e);
                    Arc::new(RwLock::new(vec![]))
                }
            }
        }
        Err(e) => {
            println!("Error reading the file: {}", e);
            Arc::new(RwLock::new(vec![]))
        }
    }
});

#[tokio::main]
async fn main() {
    // create the file if it does not exist
    let mut txt = OpenOptions::new().write(true).create(true).append(true).open("MUSICAL_LIBRARY.txt").unwrap();
    txt.write_all("".as_bytes()).unwrap();

    // create service factory
    let make_svc = make_service_fn(|_conn| async {
        Ok::<_, Infallible>(service_fn(handle_request))
    });

    // set up listening address
    let addr = ([127, 0, 0, 1], 8080).into();
    let server = Server::bind(&addr).serve(make_svc);
    println!("The server is currently listening on localhost:8080.");

    // the server shuts down when: 1.it encounters an error or 2.a shutdown signal ctrl+c is received
    tokio::select!{
        result = server => {
            if let Err(e) = result {
                eprintln!("server error: {}", e);
            }
        },
        _ = signal::ctrl_c() => {
            println!("Shutdown signal received! Starting to write into file");
        }
    }

    // write into file to make the musical library persistent
    let mut file = OpenOptions::new().write(true).create(true).truncate(true).open("MUSICAL_LIBRARY.txt").unwrap();
    let library_clone = Arc::clone(&MUSICAL_LIBRARY);
    let library = library_clone.read().await;
    let json_str = serde_json::to_string(&(*library)).unwrap();
    file.write_all(json_str.as_bytes()).unwrap();
    println!("Writing complete, shutting down..");
}

async fn handle_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    let path = req.uri().path();

    match path {
        // handling site wide visit count request
        "/count" => {
            let count_clone = Arc::clone(&VISIT_COUNT);
            let mut num = count_clone.lock().unwrap();
            *num += 1;
            let body = format!("Visit count: {}", *num);
            Ok(Response::new(Body::from(body)))
        },
        // handling request that put a new song into the musical library
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
                            let mut library = library_clone.write().await;
                            let song = Song {
                                id: (*library).len()+1,
                                title: raw_song.title,
                                artist: raw_song.artist,
                                genre: raw_song.genre,
                                play_count: 0
                            };
                            let json_string = serde_json::to_string(&song).unwrap();
                            (*library).push(song);
                            return Ok(Response::new(Body::from(json_string)));
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
            Ok(Response::builder()
                .status(400)
                .body(Body::from(""))
                .unwrap())
        },
        // handling request that search for songs with specific features
        "/songs/search" => {
            let uri = &req.uri().to_string()[..];
            match uri.rfind('?') {
                Some(pos) => {
                    // initialize the query parameters
                    let mut query: Vec<String> = vec![String::from(""); 3];
                    // parsing uri
                    let params_str = &uri[pos + 1..];
                    let params_vec: Vec<&str> = params_str.split('&').collect();
                    for pair in params_vec {
                        match pair.split_once('=') {
                            Some((param, value)) => {
                                let parsed_value = value.replace('+', " ").to_lowercase();
                                match param {
                                    "title" => {
                                        query[0] = parsed_value;
                                    },
                                    "artist" => {
                                        query[1] = parsed_value;
                                    },
                                    "genre" => {
                                        query[2] = parsed_value;
                                    },
                                    _ => {}
                                }
                            },
                            None => {
                                println!("Error parsing search request");
                            }
                        }
                    }
                    // iterate through the musical library and search
                    let library_clone = Arc::clone(&MUSICAL_LIBRARY);
                    let library = library_clone.read().await;
                    // initialize a data structure to store the search result
                    let mut result_vec: Vec<Song> = vec![];
                    for song in (*library).iter() {
                        if song.title.to_lowercase().contains(&query[0])&&
                            song.artist.to_lowercase().contains(&query[1])&&
                            song.genre.to_lowercase().contains(&query[2]) {
                            result_vec.push(song.clone());
                        }
                    }
                    let json_string = serde_json::to_string(&result_vec).unwrap();
                    return Ok(Response::new(Body::from(json_string)));
                },
                None => {
                    println!("Error parsing search request");
                }
            }
            Ok(Response::new(Body::from("[]")))
        },
        // handling request to play a song with a certain id
        s if s.starts_with("/songs/play") => {
            let uri = &(req.uri().to_string())[..];
            match uri.rfind('/') {
                Some(pos) => {
                    let id = &uri[pos + 1..];
                    let result: Result<usize, _> = id.parse();
                    match result {
                        Ok(id_num) => {
                            let library_clone = Arc::clone(&MUSICAL_LIBRARY);
                            let mut library = library_clone.write().await;
                            if let Some(song) = (*library).get_mut(id_num-1){
                                (*song).play_count += 1;
                                return Ok(Response::new(Body::from(serde_json::to_string(song).unwrap())));
                            }else {
                                println!("Could not find the song, index out of bound");
                            }
                        },
                        Err(e) => {
                            println!("Error parsing music id: {}", e);
                        }
                    }
                },
                None => {
                    println!("Error parsing play request");
                }
            }
            Ok(Response::builder()
                .status(400)
                .body(Body::from("{\"error\":\"Song not found\"}"))
                .unwrap())
        },
        _  => {
            Ok(Response::new(Body::from("Welcome to the Rust-powered web server!")))
        },
    }
}