
- FastCGI spec: https://github.com/Gedweb/rust-gfcgi/blob/master/doc/fcgi-spec.md
- Project with some nice FCGI parsing code: https://github.com/mohtar/rust-fastcgi
- Tokio streaming protocols tutorial: https://tokio.rs/docs/going-deeper/streaming/

- [x] parse bytes as FastCGI records
- [x] group FastCGI records into request header
- [x] after raising the request header, deliver body records through the request object
- [x] handle response body records and pass them back to the socket
- [x] close the socket when no requests are in flight

Now that these are done, the major pain points now are:

- No way (as far as I can tell) to indicate that tokio-proto should close the connection, so I have
  to raise an error to do this.
- Because future combinators all take a `FnOnce`, I can't have the main request logic wrapped up in
  a nice function that just takes the request and makes a result (or result future); instead, they
  have to be this ugly thing that takes a request *future* and combines with it to produce the
  result future. Yuck. Maybe if I encapsulate the request logic into a struct or something...

Remaining work to be done on this iteration:
- [x] Move the end-records logic out of the request handler and into the `FastcgiService::call`
      function.
- [x] Make a general FastcgiResponse struct for holding headers and a body stream (probably just a
      string for initial prototype) - probably necessary for the point above too.
- [ ] Try making the response body be a channel / stream.
- [x] Clean up the code and move it into the crate proper.
