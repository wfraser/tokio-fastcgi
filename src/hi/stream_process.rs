use futures::{Async, Poll, Future};
use futures::stream::Stream;

use std::mem;

enum State<S, M> {
    Processing(S, M),
    Empty
}

/// This struct processes a stream by applying a function to each element, with a mutable state,
/// until that function returns `true`, and then it returns the remaining stream and the mutable
/// state as a future.
#[must_use = "streams do nothing unless polled"]
pub struct StreamProcess<S, M, F>
        where S: Stream,
              F: FnMut(&S::Item, &mut M) -> bool
{
    state: State<S, M>,
    processor: F,
}

impl<S, M, F> StreamProcess<S, M, F>
        where S: Stream,
              F: FnMut(&S::Item, &mut M) -> bool
{
    pub fn new(stream: S, initial_state: M, f: F) -> StreamProcess<S, M, F> {
        StreamProcess {
            state: State::Processing(stream, initial_state),
            processor: f,
        }
    }
}

impl<S, M, F> Future for StreamProcess<S, M, F>
        where S: Stream,
              F: FnMut(&S::Item, &mut M) -> bool
{
    type Item = (S, M);
    type Error = S::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match mem::replace(&mut self.state, State::Empty) {
            State::Empty => panic!("cannot poll StreamProcess twice!"),
            State::Processing(mut stream, mut state) => {
                match stream.poll()? {
                    Async::Ready(Some(ref x)) => {
                        let done = (self.processor)(x, &mut state);
                        if done {
                            Ok(Async::Ready((stream, state)))
                        } else {
                            self.state = State::Processing(stream, state);
                            Ok(Async::NotReady)
                        }
                    },
                    Async::Ready(None) => {
                        Ok(Async::Ready((stream, state)))
                    },
                    Async::NotReady => {
                        self.state = State::Processing(stream, state);
                        Ok(Async::NotReady)
                    }
                }
            }
        }
    }
}
