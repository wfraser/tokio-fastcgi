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
