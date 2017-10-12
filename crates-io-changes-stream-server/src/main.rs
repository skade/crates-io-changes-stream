extern crate hyper;
extern crate futures;
extern crate crates_io_changes;
extern crate serde;
extern crate serde_json;

use futures::future::FutureResult;
use futures::Stream;

use crates_io_changes::CratesIndex;

use hyper::header::ContentLength;
use hyper::server::{Http, Request, Response, Service};
use hyper::Chunk;
use hyper::Body;

use std::path::Path;

use std::sync::Arc;

struct ChangesStream {
    index: CratesIndex
}

impl ChangesStream {
    fn new<P: AsRef<Path>>(p: P) -> ChangesStream {
        let index = CratesIndex::new(p.as_ref()).unwrap();
        ChangesStream { index: index }
    }
}

impl Service for ChangesStream {
    // boilerplate hooking up hyper's server types
    type Request = Request;
    type Response = Response<Box<Stream<Item=Chunk, Error=Self::Error>>>;
    type Error = hyper::Error;
    // The future representing the eventual Response your call will
    // resolve to. This can change to whatever Future you need.
    type Future = FutureResult<Self::Response, Self::Error>;

    fn call(&self, req: Request) -> Self::Future {
        let mut response: Self::Response = Response::new();

        if req.path() != "/_changes" {
            let body: Box<Stream<Item=_, Error=_>> = Box::new(Body::from("Try GETTING  /_changes!"));
    
            let not_found = response.with_status(hyper::StatusCode::NotFound).with_body(body);

            return futures::future::ok(not_found);
        }

        if *req.method() != hyper::Method::Get {
            let body: Box<Stream<Item=_, Error=_>> = Box::new(Body::from("Try GETTING  /_changes!"));
    
            let wrong_method = response.with_status(hyper::StatusCode::MethodNotAllowed);

            return futures::future::ok(wrong_method);
        }

        let (mut sender, body) = Body::pair();
        let stream: Box<Stream<Item=Chunk, Error=hyper::Error>> = Box::new(body);

        std::thread::spawn(move || {
           let index = CratesIndex::new("../crates-io-changes/crates.io-index").unwrap();
           let iter = index.iter().unwrap();

           for change in iter {
               let change = change.unwrap();
               let mut json = serde_json::to_string(&change).unwrap();
               json.push_str("\n");
               let res = sender.try_send(Ok(Chunk::from(json)));
               if res.is_err() {
                   break;
               }
           }
        });

        response.set_body(stream);

        futures::future::ok(response)
    }
}

fn main() {
    let addr = "127.0.0.1:3000".parse().unwrap();
    let service = Arc::new(ChangesStream::new("../crates-io-changes/crates.io-index"));

    let server = Http::new().bind(&addr, move || Ok(service.clone())).unwrap();
    server.run().unwrap();
}
