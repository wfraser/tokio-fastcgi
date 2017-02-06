#!/usr/bin/env python2.7
import socket
import fcgi_client

fcgi = fcgi_client.FCGIApp(connect = "./hello.sock")
env = {
    # This is the set of variables my nginx/1.10.2 sends:
    "QUERY_STRING": "",
    "REQUEST_METHOD": "GET",
    "CONTENT_TYPE": "",
    "CONTENT_LENGTH": "",
    "SCRIPT_FILENAME": "/srv/www/localhost/tokio-fastcgi/",
    "SCRIPT_NAME": "/tokio-fastcgi/",
    "REQUEST_URI": "/tokio-fastcgi/",
    "DOCUMENT_URI": "/tokio-fastcgi/",
    "DOCUMENT_ROOT": "/srv/www/localhost",
    "SERVER_PROTOCOL": "HTTP/2.0",
    "REQUEST_SCHEME": "https",
    "HTTPS": "on",
    "GATEWAY_INTERFACE": "CGI/1.1",
    "SERVER_SOFTWARE": "test.py",
    "REMOTE_ADDR": "::1",
    "REMOTE_PORT": "12345",
    "SERVER_ADDR": "::1",
    "SERVER_PORT": "443",
    "SERVER_NAME": "localhost",
    "PATH_INFO": "",
    "PATH_TRANSLATED": "/srv/www/localhost",
    "REDIRECT_STATUS": "200",
    "HTTP_HOST": "localhost",
    "HTTP_CACHE_CONTROL": "max-age=0",
    "HTTP_UPGRADE_INSECURE_REQUESTS": "1",
    "HTTP_USER_AGENT": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36",
    "HTTP_ACCEPT": "text/html,application/xhtml+xml,application/xml,q=0.9,image/webp,*/*;q=0.8",
    "HTTP_DNT": "1",
    "HTTP_ACCEPT_ENCODING": "gzip, deflate, sdch, br",
    "HTTP_ACCEPT_LANGUAGE": "en-US,en;q=0.8",
}
ret = fcgi(env)
print ret
