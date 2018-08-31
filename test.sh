#!/usr/bin/env bash

# clean out our environment
[ "$HOME" != "" ] && exec -c $0 $@

# This is the set of variables my nginx/1.10.2 sends:
export QUERY_STRING=""
export REQUEST_METHOD="GET"
export CONTENT_TYPE=""
export CONTENT_LENGTH=""
export SCRIPT_FILENAME="/srv/www/localhost/tokio-fastcgi/"
export SCRIPT_NAME="/tokio-fastcgi/"
export REQUEST_URI="/tokio-fastcgi/"
export DOCUMENT_URI="/tokio-fastcgi/"
export DOCUMENT_ROOT="/srv/www/localhost"
export SERVER_PROTOCOL="HTTP/2.0"
export REQUEST_SCHEME="https"
export HTTPS="on"
export GATEWAY_INTERFACE="CGI/1.1"
export SERVER_SOFTWARE="test.py"
export REMOTE_ADDR="::1"
export REMOTE_PORT="12345"
export SERVER_ADDR="::1"
export SERVER_PORT="443"
export SERVER_NAME="localhost"
export PATH_INFO=""
export PATH_TRANSLATED="/srv/www/localhost"
export REDIRECT_STATUS="200"
export HTTP_HOST="localhost"
export HTTP_CACHE_CONTROL="max-age=0"
export HTTP_UPGRADE_INSECURE_REQUESTS="1"
export HTTP_USER_AGENT="Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36"
export HTTP_ACCEPT="text/html,application/xhtml+xml,application/xml,q=0.9,image/webp,*/*;q=0.8"
export HTTP_DNT="1"
export HTTP_ACCEPT_ENCODING="gzip, deflate, sdch, br"
export HTTP_ACCEPT_LANGUAGE="en-US,en;q=0.8"

cgi-fcgi -bind -connect hello.sock
