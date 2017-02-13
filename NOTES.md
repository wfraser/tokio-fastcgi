
- FastCGI spec: https://github.com/Gedweb/rust-gfcgi/blob/master/doc/fcgi-spec.md
- Project with some nice FCGI parsing code: https://github.com/mohtar/rust-fastcgi
- Tokio streaming protocols tutorial: https://tokio.rs/docs/going-deeper/streaming/

- [x] parse bytes as FastCGI records
- [x] group FastCGI records into request header
- [x] after raising the request header, deliver body records through the request object
- [x] handle response body records and pass them back to the socket
- [x] close the socket when no requests are in flight

Now that these are done, the major pain points now are:

- ~~No way (as far as I can tell) to indicate that tokio-proto should close the connection, so I
  have to raise an error to do this.~~
  I added a bit of a better hack in 459bf5f3a9bac88d6eeb14e3efc1b59767e5cf85 that watches the
  requests as they are sent and received in the transport and closes the connection by consuming
  the IO object. I still feel like this should be easier... see:
    https://github.com/tokio-rs/tokio-proto/issues/141

Remaining work to be done on this iteration:
- [x] Move the end-records logic out of the request handler and into the `FastcgiService::call`
      function.
- [x] Make a general FastcgiResponse struct for holding headers and a body stream (probably just a
      string for initial prototype) - probably necessary for the point above too.
- [x] Clean up the code and move it into the crate proper.
- [ ] Try making the response body be a channel / stream.
- [ ] Relatedly, make sure that responses > 65535 bytes are sent correctly.
- [ ] Stream in the request body as EasyBufs instead of FastcgiRecords.
