import socket
import fcgi_client

fcgi = fcgi_client.FCGIApp(connect = "./hello.sock")
env = {
    "SERVER_NAME": "zomg.wtf",
    "REQUEST_METHOD": "GET",
    "GATEWAY_INTERFACE": "CGI/1.1",
    "REQUEST_URI": "/",
    "CONTENT-LENGTH": "0",
    "CONTENT-TYPE": "",
}
ret = fcgi(env)
print ret
