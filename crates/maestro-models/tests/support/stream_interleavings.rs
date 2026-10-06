use maestro_models::{AsyncIterator, EventStream};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

#[cfg(not(target_arch = "wasm32"))]
type Next = Pin<Box<dyn Future<Output = Option<i32>> + Send>>;
#[cfg(target_arch = "wasm32")]
type Next = Pin<Box<dyn Future<Output = Option<i32>>>>;

struct Reading {
    future: Next,
    value: Value,
    observed: bool,
}
#[derive(Default)]
struct Consumers {
    cursors: BTreeMap<char, AsyncIterator<i32>>,
    reads: Vec<Reading>,
    extraction: bool,
}
impl Consumers {
    fn next(&mut self, cursor: char, mode: char) {
        let future = self.cursors.get_mut(&cursor).unwrap().next();
        if mode == 'd' {
            drop(future);
        } else {
            self.reads.push(Reading {
                future,
                value: json!("pending"),
                observed: mode == 'n',
            });
        }
    }
}

pub fn check_corpus() {
    let corpus: Value =
        serde_json::from_str(include_str!("../fixtures/stream_interleavings.json")).unwrap();
    assert!(corpus.as_array().unwrap().len() >= 25);
    for case in corpus.as_array().unwrap() {
        let consumers = Arc::new(Mutex::new(Consumers::default()));
        let extracting = consumers.clone();
        let stream = EventStream::new(
            Arc::new(|value: &i32| Ok(*value < 0)),
            Arc::new(move |value| {
                let mut consumers = extracting.lock().unwrap();
                if consumers.extraction {
                    consumers.next('A', 'n');
                }
                Ok(*value)
            }),
        );
        let mut result = stream.result();
        let mut final_value = json!("pending");
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let op = step["op"].as_str().unwrap();
            let mode = op.chars().next().unwrap();
            match mode {
                'c' => {
                    consumers
                        .lock()
                        .unwrap()
                        .cursors
                        .insert(op.chars().nth(1).unwrap(), stream.iter());
                }
                'n' | 'd' | 'u' => consumers
                    .lock()
                    .unwrap()
                    .next(op.chars().nth(1).unwrap(), mode),
                'p' => stream.push(op[1..].parse().unwrap()).unwrap(),
                'e' => stream.end(if op.len() == 1 {
                    None
                } else {
                    Some(op[1..].parse().unwrap())
                }),
                'x' => consumers.lock().unwrap().extraction = true,
                'y' => {
                    let mut cx = Context::from_waker(Waker::noop());
                    if let Poll::Ready(value) = result.as_mut().poll(&mut cx) {
                        final_value = json!(value);
                    }
                    let mut consumers = consumers.lock().unwrap();
                    for reading in &mut consumers.reads {
                        if reading.observed
                            && reading.value == json!("pending")
                            && let Poll::Ready(value) = reading.future.as_mut().poll(&mut cx)
                        {
                            reading.value =
                                value.map_or_else(|| json!("done"), |value| json!(value));
                        }
                    }
                }
                _ => panic!("unknown operation {op}"),
            }
            let reads: Vec<_> = consumers
                .lock()
                .unwrap()
                .reads
                .iter()
                .filter(|read| read.observed)
                .map(|read| read.value.clone())
                .collect();
            assert_eq!(
                json!(reads),
                step["reads"],
                "{} step {index}: {op}",
                case["name"]
            );
            assert_eq!(
                final_value, step["result"],
                "{} step {index}: {op} result",
                case["name"]
            );
        }
    }
}
