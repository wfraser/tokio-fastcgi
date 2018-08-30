This was made as an experiment, to test the usability of `tokio-proto` to make
a useful network service.

Unfortunately, `tokio-proto` has been abandoned as the Rust community's async /
futures direction has changed yet again, so this code probably doesn't have
much of a future. See https://github.com/tokio-rs/tokio/issues/118 for a
discussion about the fate of `tokio-proto`.

That said, it works great, and you can check out the `examples/` directory to
see how it can be used. Especially `base64ify` and `countdown` really
demonstrate how FastCGI and proper asynchronous I/O can be used effectively.

`base64ify` converts any POSTed content to Base64 on the fly, as it reads it
from the browser.

`countdown` counts down from 10, with one-second delays in between, but flushes
its buffers in such a way that the browser should be able to see each number
appear individually.

The examples at start-up all create a UNIX domain socket file in the current
directory named `hello.sock`. If you configure NGINX or any other
FastCGI-capable web browser to forward requests to that socket, you can see it
in action.

The code is internally structured into two layers:

The low-level codec (implemented mostly in `src/lowlevel.rs`) takes byte
buffers and interprets them as raw FastCGI records, and vice-versa.

The high-level layer (implemented under `src/hi/`) takes these FastCGI records
and implements the actual FastCGI protocol on top of them, parsing out headers,
turning the records into a stream of *requests*, and turning *responses* back
into records.

`tokio-proto` takes care of putting the low-level codec and the high-level
protocol together. The web server frontend feeds bytes into the socket, the
bytes get read and parsed into into records, and then records into requests.
Your program gets called with the request, it produces a response (potentially
incrementally in chunks), and it takes the response and turns it into multiple
records, and then turns those records into bytes to be fed to the web server
frontend.

As a user of this library, you probably want to interact with this high-level
layer. Do that by implementing the `FastcgiRequestHandler` trait, which results
in your code being passed a `FastcgiRequest` for each request, and through
which you send response headers and body stream, via futures.

(Alternatively, you can ignore all that and get the raw stream of FastCGI
records by using `FastcgiLowlevelCodec` directly, though this is probably not
super useful. The example in `examples/lowlevel.rs` does just that, and it
doesn't even send a response to the browser because it's too big a pain to do
it that way.)
