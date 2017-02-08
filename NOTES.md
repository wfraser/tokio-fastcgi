
- FastCGI spec: https://github.com/Gedweb/rust-gfcgi/blob/master/doc/fcgi-spec.md
- Project with some nice FCGI parsing code: https://github.com/mohtar/rust-fastcgi
- Tokio streaming protocols tutorial: https://tokio.rs/docs/going-deeper/streaming/

- [x] parse bytes as FastCGI records
- [x] group FastCGI records into request header
- [ ] after raising the request header, deliver body records through the request object
- [ ] handle response body records and pass them back to the socket
- [ ] close the socket when no requests are in flight

Observations: implementing the control flow in a `filter_map` isn't going to work well because the
return value of handling each record can't be a future, and for things like the body (stdin/data)
records, it should be the result of doing a Sender::send(...), which is a
`Future<Item = Sink, ...>`.

Instead, `FastcgiServer` will probably have to implement `Stream<Item = FastcgiRequest>` manually,
by having its `poll` call the underlying `Framed` stream's `poll` in a loop until the request
headers are all received. Subsequent calls to `poll` will drive delivery of body records.

Probably best to check what `Framed` does in its `poll`, since it is doing something similar.

---

Update 2017-02-07: I have the last 3 bullet points implemented in a hacky way now at least, using
tokio-proto again.

The major pain points now are:

- No way (as far as I can tell) to indicate that tokio-proto should close the connection, so I have
  to raise an error to do this.
- Because future combinators all take a `FnOnce`, I can't have the main request logic wrapped up in
  a nice function that just takes the request and makes a result (or result future); instead, they
  have to be this ugly thing that takes a request *future* and combines with it to produce the
  result future. Yuck. Maybe if I encapsulate the request logic into a struct or something...

Remaining work to be done on this iteration:
- [ ] Move the end-records logic out of the request handler and into the `FastcgiService::call`
      function.
- [ ] Make a general FastcgiResponse struct for holding headers and a body stream (probably just a
      string for initial prototype) - probably necessary for the point above too.
- [ ] Clean up the code and move it into the crate proper.
